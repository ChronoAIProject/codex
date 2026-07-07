#[cfg(target_os = "windows")]
const SEE_MASK_NOCLOSEPROCESS: u32 = windows_sys::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
#[cfg(not(target_os = "windows"))]
const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;

#[cfg(target_os = "windows")]
const SEE_MASK_FLAG_NO_UI: u32 = windows_sys::Win32::UI::Shell::SEE_MASK_FLAG_NO_UI;
#[cfg(not(target_os = "windows"))]
const SEE_MASK_FLAG_NO_UI: u32 = 0x0000_0400;

pub(crate) fn setup_exe_shell_execute_mask() -> u32 {
    SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI
}

#[cfg(test)]
mod tests {
    use super::SEE_MASK_FLAG_NO_UI;
    use super::SEE_MASK_NOCLOSEPROCESS;
    use pretty_assertions::assert_eq;

    #[test]
    fn setup_exe_shell_execute_mask_suppresses_shell_error_ui() {
        let mask = super::setup_exe_shell_execute_mask();

        assert_eq!(mask & SEE_MASK_NOCLOSEPROCESS, SEE_MASK_NOCLOSEPROCESS);
        assert_eq!(mask & SEE_MASK_FLAG_NO_UI, SEE_MASK_FLAG_NO_UI);
    }
}
