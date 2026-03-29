use crate::profile::AppProfile;
use crate::types::CaptureMethod;

pub(crate) fn prioritize_profile_method(
    mut methods: Vec<CaptureMethod>,
    profile: Option<&AppProfile>,
) -> Vec<CaptureMethod> {
    let Some(preferred_method) = profile.and_then(|profile| profile.last_success_method) else {
        return methods;
    };

    let Some(index) = methods
        .iter()
        .position(|method| *method == preferred_method)
    else {
        return methods;
    };

    if index > 0 {
        let preferred_method = methods.remove(index);
        methods.insert(0, preferred_method);
    }

    methods
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{AppProfile, TriState};

    #[test]
    fn moves_profile_method_to_front_when_present() {
        let profile = AppProfile {
            bundle_id: "com.example".into(),
            ax_supported: TriState::Unknown,
            clipboard_borrow_supported: TriState::Unknown,
            last_success_method: Some(CaptureMethod::ClipboardBorrow),
            last_failure_kind: None,
        };

        let methods = prioritize_profile_method(
            vec![
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
                CaptureMethod::ClipboardBorrow,
            ],
            Some(&profile),
        );

        assert_eq!(
            methods,
            vec![
                CaptureMethod::ClipboardBorrow,
                CaptureMethod::AccessibilityPrimary,
                CaptureMethod::AccessibilityRange,
            ]
        );
    }
}
