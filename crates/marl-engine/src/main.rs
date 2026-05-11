fn main() {
    let cfg = marl_config::Config::load();
    #[cfg(feature = "gpu")]
    let use_gpu = std::env::args().any(|arg| arg == "--gpu-diffusion");
    #[cfg(not(feature = "gpu"))]
    let use_gpu = false;
    marl_sim::run(cfg, use_gpu);
}
