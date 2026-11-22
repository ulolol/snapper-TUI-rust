/// Format a byte count into a compact human-readable string matching Snapper TUI conventions.
pub fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;

    let b = bytes as f64;
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if b < MIB {
        format!("{:.1}K", b / KIB)
    } else if b < GIB {
        format!("{:.1}M", b / MIB)
    } else if b < TIB {
        format!("{:.1}G", b / GIB)
    } else {
        format!("{:.1}T", b / TIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(2048), "2.0K");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0M");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0G");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024 * 1024), "3.0T");
    }
}
