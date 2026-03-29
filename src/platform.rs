use crate::types::CaptureMethod;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformCapabilities {
    supported: [bool; 4],
}

impl PlatformCapabilities {
    pub fn new(supported: &[CaptureMethod]) -> Self {
        let mut capabilities = [false; 4];
        for &method in supported {
            capabilities[method_index(method)] = true;
        }
        Self {
            supported: capabilities,
        }
    }

    pub fn supports(&self, method: CaptureMethod) -> bool {
        self.supported[method_index(method)]
    }
}

const fn method_index(method: CaptureMethod) -> usize {
    match method {
        CaptureMethod::AccessibilityPrimary => 0,
        CaptureMethod::AccessibilityRange => 1,
        CaptureMethod::ClipboardBorrow => 2,
        CaptureMethod::SyntheticCopy => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_returns_true_for_enabled_methods() {
        let capabilities = PlatformCapabilities::new(&[
            CaptureMethod::AccessibilityPrimary,
            CaptureMethod::ClipboardBorrow,
        ]);

        assert!(capabilities.supports(CaptureMethod::AccessibilityPrimary));
        assert!(capabilities.supports(CaptureMethod::ClipboardBorrow));
    }

    #[test]
    fn supports_returns_false_for_disabled_methods() {
        let capabilities = PlatformCapabilities::new(&[
            CaptureMethod::AccessibilityPrimary,
            CaptureMethod::ClipboardBorrow,
        ]);

        assert!(!capabilities.supports(CaptureMethod::AccessibilityRange));
        assert!(!capabilities.supports(CaptureMethod::SyntheticCopy));
    }
}
