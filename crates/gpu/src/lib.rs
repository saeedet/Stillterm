//! Experimental corridor: shared deterministic rain sheets and a GPU projection.
pub mod scene;
pub const CORRIDOR_SHADER: &str = include_str!("corridor.wgsl");
#[cfg(target_os = "macos")]
pub mod renderer;

#[cfg(test)]
mod tests {
    #[test]
    fn corridor_shader_parses_and_validates_without_a_gpu() {
        let module = naga::front::wgsl::parse_str(super::CORRIDOR_SHADER).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }
}
