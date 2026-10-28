/// How to join the `arch-sense` group, which is what grants hardware access:
/// the command, and the step after it that people forget.
pub(crate) const JOIN_GROUP: &str = "`sudo usermod -aG arch-sense $USER`, then log out and back in";

/// What to tell a user who hits a permission error. The package grants the
/// `arch-sense` group access to the hardware; membership is the one manual step.
pub(crate) fn setup_hint() -> String {
    format!("join the `arch-sense` group with {JOIN_GROUP}")
}
