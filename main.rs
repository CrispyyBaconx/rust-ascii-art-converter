use image::GenericImageView;
use std::thread::available_parallelism;
use std::num::NonZero;

fn get_char(intensity: u8) -> char {
    let index = (intensity / 32) as usize;
    let chars = " .,-~+=@"; 
    chars.chars().nth(index).unwrap()
}

fn get_image_ascii(dir: &str, scale: u32) -> String {
    let img = image::open(dir).unwrap();
    let (width, height) = img.dimensions();
    let mut result = String::new();
    
    for y in (0..height).step_by(scale as usize * 2) {
        for x in (0..width).step_by(scale as usize) {
            let pixel = img.get_pixel(x, y);
            let mut intensity = pixel[0] / 3 + pixel[1] / 3 + pixel[2] / 3;
            if pixel[3] == 0 {
                intensity = 0;
            }
            result.push(get_char(intensity));
        }
        
        if y % (scale * 2) == 0 {
            result.push('\n');
        }
    }
    
    result
}

fn get_image(dir: &str, scale: u32, output: &str) {
    use std::io::Write;

    let ascii_art = get_image_ascii(dir, scale);
    let mut file = std::fs::File::create(output).unwrap();
    file.write_all(ascii_art.as_bytes()).unwrap();
}

fn get_video(dir: &str, scale: u32, output: &str) {
    use std::process::Command;
    use std::fs;
    use std::time::Instant;
    use imageproc::drawing::draw_text_mut;
    use ab_glyph::{FontRef, PxScale};
    use rayon::prelude::*;
    use rayon::ThreadPoolBuilder;

    // Setup thread pool
    let threads = available_parallelism().unwrap_or(NonZero::new(8).unwrap()).get().into();
    println!("Using {} threads", threads);
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .unwrap();

    // Start Timer
    let start_time = Instant::now();
    
    // Create temp directory for frames
    let temp_dir = "temp_frames";
    let ascii_frames_dir = "ascii_frames";
    fs::create_dir_all(temp_dir).expect("Failed to create temp directory");
    fs::create_dir_all(ascii_frames_dir).expect("Failed to create ASCII frames directory");
    
    // Get framerate from source video
    let framerate = Command::new("ffprobe")
        .args(&["-v", "0", "-of", "csv=p=0", "-select_streams", "v:0", "-show_entries", "stream=r_frame_rate", dir])
        .output()
        .expect("Failed to get framerate");
    let framerate = String::from_utf8(framerate.stdout).unwrap();
    let framerate = framerate
        .split(',')
        .next()
        .unwrap();

    // Extract frames from video
    let extract_status = Command::new("ffmpeg")
        .args(&["-i", dir, "-vf", &format!("fps={}", framerate), &format!("{}/frame%04d.png", temp_dir)])
        .status()
        .expect("Failed to execute ffmpeg for frame extraction");
    
    if !extract_status.success() {
        eprintln!("Failed to extract frames from video");
        return;
    }
    
    let font_path = if cfg!(windows) {
        "C:\\Windows\\Fonts\\consola.ttf" // Windows
    } else if cfg!(target_os = "macos") {
        "/Library/Fonts/Courier New.ttf"  // macOS
    } else {
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf" // Linux
    };

    let font_data = std::fs::read(font_path).expect("Failed to load font");
    let font = FontRef::try_from_slice(&font_data).expect("Failed to parse font");
    
    // Get all frame paths
    let paths: Vec<_> = fs::read_dir(temp_dir)
        .unwrap()
        .map(|r| r.unwrap().path())
        .collect();
    
    let total_frames = paths.len();
    println!("Processing {} frames in parallel...", total_frames);
    
    // Use parallelism to go fasta
    paths.par_iter().enumerate().for_each(|(i, path)| {
        let path_str = path.to_str().unwrap();
        
        // Convert frame
        let ascii_art = get_image_ascii(path_str, scale);
        let lines: Vec<&str> = ascii_art.lines().collect();
        
        // Skip empty results
        if lines.is_empty() {
            return;
        }
        
        // Get original image dimensions for reference
        let img = image::open(path_str).unwrap();
        let (width, height) = img.dimensions();
        
        // Calculate font size based on image dimensions & ASCII art size
        let char_width = width as f32 / lines[0].len() as f32;
        let char_height = height as f32 / lines.len() as f32;
        let font_size = char_height.min(char_width * 2.0) * 0.9;
        
        // Create the image
        let mut ascii_image = image::RgbImage::new(width, height);
        for pixel in ascii_image.pixels_mut() {
            *pixel = image::Rgb([0, 0, 0]);
        }
        
        let scale = PxScale::from(font_size);
        
        // Draw each line
        for (y, line) in lines.iter().enumerate() {
            let y_pos = (y as f32 * char_height) as i32;
            
            draw_text_mut(
                &mut ascii_image, 
                image::Rgb([255, 255, 255]),
                0, 
                y_pos, 
                scale,
                &font,
                line
            );
        }
        
        // Save the image
        let ascii_path = format!("{}/ascii_frame{:04}.png", ascii_frames_dir, i+1);
        ascii_image.save(&ascii_path).expect("Failed to save ASCII image");
        
        println!("Processed frame {}/{}", i+1, total_frames);
    });
    
    println!("Frame processing complete. Creating video...");
    
    // Combine frames into video
    let combine_status = Command::new("ffmpeg")
        .args(&["-framerate", framerate, "-i", &format!("{}/ascii_frame%04d.png", ascii_frames_dir), 
               "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", output])
        .status()
        .expect("Failed to execute ffmpeg for video creation");
    
    if !combine_status.success() {
        eprintln!("Failed to create ASCII video");
        return;
    }
    
    // Clean up temp directories
    fs::remove_dir_all(temp_dir).unwrap_or_else(|_| println!("Warning: Failed to remove temp directory"));
    fs::remove_dir_all(ascii_frames_dir).unwrap_or_else(|_| println!("Warning: Failed to remove ASCII frames directory"));
    
    println!("Video processing complete. Output saved to: {}", output);
    println!("Done in {:?}!", start_time.elapsed());
}

fn main() {
    // get image from args
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        println!("Usage: {} <path> <type (image/video)> <scale (default: 1)> <output_name (default: output.txt/mp4)>", args[0]);
        return;
    }

    // check if the path exists on disk
    if !std::path::Path::new(&args[1]).exists() {
        println!("Error: File does not exist");
        return;
    }

    if args[2] == "image" {
        let dir = &args[1];
        let scale = args[3].parse::<u32>().unwrap_or(1);
        let output = if args.len() > 4 { &args[4] } else { "output.txt" };
        get_image(dir, scale, output);
    } else if args[2] == "video" {
        let dir = &args[1];
        let scale = args[3].parse::<u32>().unwrap_or(1);
        let output = if args.len() > 4 { &args[4] } else { "output.mp4" };
        get_video(dir, scale, output);
    } else {
        println!("Usage: {} <path> <type (image/video)> <scale (default: 1)> <output_name (default: output.txt/mp4)>", args[0]);
        return;
    }
}

