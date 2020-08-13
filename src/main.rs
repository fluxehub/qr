mod qr;

use crate::qr::qr::QR;
use std::env;

fn main() {
    // Basic command-line parser
    // TODO: replace with something c o o l e r 
    let args: Vec<String> = env::args().collect();
    let input = args[1].clone();
    let mut code = QR::new(input);
    code.generate();

    if args.len() == 3 {
        code.save_image(args[2].clone(), 1000)
    } else if args.len() > 3 {
        code.save_image(args[2].clone(), args[3].parse().unwrap())
    }
}
