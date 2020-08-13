mod qr;

use crate::qr::qr::QR;

fn main() {
    let mut code = QR::new(String::from("My name jeff"));
    code.generate();
    code.save_image(String::from("qr.png"), 1000)
}
