use super::build_default_dacl_sids;
use pretty_assertions::assert_eq;
use std::ffi::c_void;

#[test]
fn default_dacl_includes_extra_user_sid_for_elevated_tokens() {
    let logon = 1usize as *mut c_void;
    let everyone = 2usize as *mut c_void;
    let capability = 3usize as *mut c_void;
    let user_sid = 4usize as *mut c_void;

    let dacl_sids = build_default_dacl_sids(logon, everyone, &[capability], &[user_sid]);

    assert_eq!(dacl_sids, vec![logon, everyone, capability, user_sid]);
}

#[test]
fn default_dacl_deduplicates_extra_sids() {
    let logon = 1usize as *mut c_void;
    let everyone = 2usize as *mut c_void;
    let capability = 3usize as *mut c_void;

    let dacl_sids =
        build_default_dacl_sids(logon, everyone, &[capability], &[capability, everyone]);

    assert_eq!(dacl_sids, vec![logon, everyone, capability]);
}
