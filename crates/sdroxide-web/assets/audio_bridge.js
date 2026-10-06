// Audio bridge for the sdroxide web client.
//
// Downlink: wasm pushes mono 48 kHz PCM (Float32Array) -> playback worklet.
// Uplink: capture worklet posts mic blocks here; wasm polls pullMic().
//
// The AudioContext can only start after a user gesture, so everything is
// initialized lazily on the first one.

(function () {
    let ctx = null;
    let player = null;
    let micChunks = [];
    let micWanted = false;
    let micStarting = false;
    let micReady = false;
    let initStarted = false;

    // AudioWorklet and getUserMedia are both secure-context-only, and the whole
    // audio path runs through them. Over plain http a browser only treats
    // localhost as secure, so a page opened at http://<lan-ip>:4950 gets no
    // receive audio and no microphone at all. That is a browser rule, not
    // something the server can opt out of, so say so instead of failing mute.
    const INSECURE = !window.isSecureContext;

    function warnInsecure() {
        // The solar view is a silent viewer; an audio warning there is noise.
        if (location.search.includes("view=solar")) return;
        console.warn(
            "sdroxide audio: this page is not a secure context, so the browser " +
            "withholds AudioWorklet and the microphone. Receive audio and " +
            "microphone transmit are unavailable. Reach the server over HTTPS " +
            "(a reverse proxy or a VPN), or via localhost through an SSH tunnel."
        );
        const show = () => {
            const bar = document.createElement("div");
            bar.style.cssText =
                "position:fixed;left:0;right:0;top:0;z-index:9999;padding:8px 12px;" +
                "font:13px system-ui,sans-serif;background:#4a3000;color:#ffd479;" +
                "border-bottom:1px solid #7a5000;cursor:pointer;" +
                // The page disables selection; this text is worth copying.
                "-webkit-user-select:text;user-select:text";
            bar.id = "sdroxide-audio-context-warning";
            bar.setAttribute("data-sdroxide-source", "No audio: {} is not a secure origin, so this browser withholds audio playback and microphone access. Use HTTPS or an SSH tunnel to localhost. (Click to dismiss.)");
            bar.setAttribute("data-sdroxide-origin", location.protocol + "//" + location.host);
            bar.textContent =
                "No audio: " + location.protocol + "//" + location.host +
                " is not a secure origin, so this browser withholds audio playback " +
                "and microphone access. Use HTTPS or an SSH tunnel to localhost. " +
                "(Click to dismiss.)";
            bar.addEventListener("click", () => bar.remove(), { once: true });
            document.body.appendChild(bar);
        };
        // This script runs from <head>, so <body> may not exist yet.
        if (document.body) show();
        else window.addEventListener("DOMContentLoaded", show, { once: true });
    }

    async function init() {
        if (initStarted) return;
        initStarted = true;
        try {
            ctx = new AudioContext({ sampleRate: 48000 });
            // iOS drops the context on any audio-session interruption (a call,
            // another app, backgrounding) and never comes back on its own. It
            // also has a non-standard "interrupted" state, so test against
            // "running" rather than listing the states we know about.
            ctx.onstatechange = () => {
                console.log("sdroxide audio: context state", ctx.state);
                if (ctx.state !== "running") resumeCtx();
            };
            // Start the module fetch but do NOT await it yet: resume() has to
            // be called while the gesture's transient activation is still
            // alive, and on WebKit that is gone by the time an await resolves.
            const moduleReady = ctx.audioWorklet.addModule("pcm_worklet.js");
            resumeCtx();
            await moduleReady;
            player = new AudioWorkletNode(ctx, "pcm-player", {
                outputChannelCount: [1],
            });
            player.connect(ctx.destination);
            // Second chance, now that there is something to hear.
            resumeCtx();
            console.log("sdroxide audio: playback ready at", ctx.sampleRate, "Hz");
        } catch (e) {
            console.warn("sdroxide audio init failed:", e);
        }
    }

    function resumeCtx() {
        if (!ctx || ctx.state === "running") return;
        // Outside a user gesture this rejects; the next gesture retries.
        ctx.resume().catch(() => {});
    }

    // Opening the microphone is deliberately deferred until the app actually
    // wants to transmit. On iOS getUserMedia switches the audio session into
    // play-and-record, which attenuates and reroutes playback -- so requesting
    // it up front makes receive audio quiet or silent for every listener who
    // never keys up, on top of prompting them for a mic they will not use.
    async function startMic() {
        if (micStarting || micReady || !ctx) return;
        micStarting = true;
        try {
            // Raw capture, explicitly. A browser defaults all three of these
            // ON, because its idea of a microphone is a conference call: the
            // echo canceller removes whatever the page is also playing, the
            // noise suppressor removes anything that does not look like
            // speech, and the gain control rides the level.
            //
            // Every one of them is wrong on a transmitter. A transmitter must
            // send what the microphone heard -- the suppressor alone will gate
            // a CW note or a digital tone to nothing, since neither looks like
            // speech to it. The echo canceller is worse still: this page plays
            // receive audio through the same AudioContext, so the band becomes
            // the far-end reference and the operator's voice becomes the thing
            // it is built to subtract, which is a microphone that reads open
            // and transmits silence (issue #493; receive audio is also held
            // off for the length of an over now, see `RemoteController`).
            //
            // Plain values rather than `exact`: these are preferences, and a
            // device that cannot turn its processing off should still open.
            const stream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    channelCount: 1,
                    echoCancellation: false,
                    noiseSuppression: false,
                    autoGainControl: false,
                },
            });
            // What the browser actually gave us, named in the log so a report
            // of a silent transmitter does not need a second round trip.
            const track = stream.getAudioTracks()[0];
            const got = (track && track.getSettings) ? track.getSettings() : {};
            console.log(
                "sdroxide audio: microphone", (track && track.label) || "(unnamed)",
                "rate", got.sampleRate || "(context)",
                "channels", got.channelCount || 1,
                "aec", got.echoCancellation, "ns", got.noiseSuppression,
                "agc", got.autoGainControl
            );
            const src = ctx.createMediaStreamSource(stream);
            const capture = new AudioWorkletNode(ctx, "mic-capture");
            capture.port.onmessage = (ev) => {
                micChunks.push(ev.data);
                // Bound: ~1 s of backlog.
                while (micChunks.length > 400) micChunks.shift();
            };
            src.connect(capture);
            // A Web Audio node is only rendered if it has a path through to the
            // destination, and MicCapture is driven entirely by `process()` —
            // so with no onward connection it was never scheduled, its
            // `postMessage` never fired, and the mic fed nothing while logging
            // "mic ready". The processor writes no output, so this path carries
            // silence and cannot feed back.
            capture.connect(ctx.destination);
            micReady = true;
            console.log("sdroxide audio: mic ready");
        } catch (e) {
            // Safari wants transient activation for getUserMedia, and the TX
            // request reaches us a frame or more after the tap that caused it.
            // Leave micWanted set so the next gesture tries again.
            console.warn("sdroxide audio: no microphone (will retry):", e);
        } finally {
            micStarting = false;
        }
    }

    function onGesture() {
        init();
        resumeCtx();
        if (micWanted) startMic();
    }

    if (INSECURE) {
        warnInsecure();
    } else {
        // Capture phase, and not just "click": eframe's canvas handlers call
        // preventDefault() on touchstart -- which stops WebKit synthesizing the
        // compatibility mouse events, so a touch device never fires "click" at
        // all -- and stopPropagation() on touchstart and pointerdown, which
        // keeps bubble-phase window listeners from ever running. Capture runs
        // before the canvas target, so neither can hide the gesture from us.
        const opts = { capture: true, passive: true };
        for (const ev of ["pointerdown", "touchstart", "touchend", "click", "keydown"]) {
            window.addEventListener(ev, onGesture, opts);
        }
        // iOS suspends the context when the tab goes to the background.
        document.addEventListener("visibilitychange", () => {
            if (!document.hidden) resumeCtx();
        });
    }

    window.sdroxideAudio = {
        pushPcm: function (pcm) {
            if (player) {
                // Copy: the wasm memory view is invalidated on return.
                player.port.postMessage(new Float32Array(pcm));
            }
        },
        pullMic: function () {
            if (micChunks.length === 0) return new Float32Array(0);
            let total = 0;
            for (const c of micChunks) total += c.length;
            const out = new Float32Array(total);
            let off = 0;
            for (const c of micChunks) {
                out.set(c, off);
                off += c.length;
            }
            micChunks = [];
            return out;
        },
        setMicActive: function (active) {
            micWanted = !!active;
            if (micWanted) startMic();
        },
    };
})();
