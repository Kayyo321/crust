/**
* Ko, (Baczynski-Bruce) 2026; Apache 2.0
*/

fn main() {
    let code = crust::driver::run(std::env::args().skip(1).collect());
    std::process::exit(code);
}
