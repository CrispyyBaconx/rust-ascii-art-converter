use image::GenericImageView;

fn get_char(intensity: u8) -> char {
    let index = (intensity / 32) as usize;
    let chars = " .,-~+=@"; 
    chars.chars().nth(index).unwrap()
}

fn get_image(dir: &str, scale: u32) {
    let img = image::open(dir).unwrap();
    let (width, height) = img.dimensions();
    for y in (0..height).step_by(scale as usize * 2) {
        for x in (0..width).step_by(scale as usize) {
            let pixel = img.get_pixel(x, y);
                let mut intensity = pixel[0] / 3 + pixel[1] / 3 + pixel[2] / 3;
                if pixel[3] == 0 {
                    intensity = 0;
                }
            print!("{}", get_char(intensity));
        }

        if y % (scale * 2) == 0 {
            println!(""); // new line
        }
    }
}

fn main() {
    // get image from args
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        println!("Usage: {} <image_path> <scale (default: 1)>", args[0]);
        return;
    }

    let dir = &args[1];
    let scale = args[2].parse::<u32>().unwrap_or(1);
    get_image(dir, scale);
}

