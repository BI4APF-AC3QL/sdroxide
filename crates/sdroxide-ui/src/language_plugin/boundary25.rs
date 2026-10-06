//! Fixed presentation namespaces only. Protocol command arguments remain raw.
pub fn js8_command_display(command: &str) -> String {
    super::scope_text("boundary25.js8.command.",command)
}
pub fn hpsdr_header_display(header: &str) -> String {
    super::scope_text("boundary25.hpsdr.header.",header)
}
pub fn meter_row_display(label: &str) -> String {
    super::scope_text("boundary25.meter.row.",label)
}

pub fn js8_speed_display(speed: sdroxide_types::Js8Speed) -> String {
    super::scope_text("boundary25.js8.speed.",speed.label())
}
