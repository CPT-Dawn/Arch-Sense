/// What to tell a user who hits a permission error. The package grants the
/// `arch-sense` group access to the hardware; membership is the one manual step.
pub(crate) fn setup_hint() -> &'static str {
    "join the `arch-sense` group with `sudo usermod -aG arch-sense $USER`, then log out and back in"
}
