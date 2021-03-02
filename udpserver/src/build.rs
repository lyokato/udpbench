fn main() {
    cc::Build::new()
        .flag("-Wall")
        .file("src/sockcbpf.c")
        .compile("sockcbpf");
}
