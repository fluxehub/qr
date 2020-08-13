pub mod qr {
    use array2d::Array2D;
    use reed_solomon::Encoder;
    use std::cmp::Ordering;
    use std::process::exit;
    use image;

    type RawImage = Array2D<u8>;

    pub struct QR {
        pub size: usize,
        pub version: usize,

        data: Vec<u8>,
        payload: Vec<u8>,
        image: RawImage, 
        masked: RawImage,
    }

    impl QR {
        pub fn new(input: String) -> QR {
            // Table of capacities for versions 1-10 at Q error correction level
            let capacity_table: [usize; 10] = [11, 20, 32, 46, 60, 74, 86, 108, 130, 151];
            let mut version = 0;

            for v in 0..capacity_table.len() {
                if capacity_table[v] > input.len() {
                    version = v + 1;
                    break;
                } 
            }

            if version > 2 {
                println!("Message is too long! (Must be 20 characters or less)");
                exit(0);
            }

            println!("Generating version {} QR code", version);

            // Initialise the data with the mode indicator, 0100 (byte mode)
            let mut data: Vec<u8> = vec![4];

            // Create header for QR code
            if version < 10 {
                let char_count = input.len() as u8;
                data.push(char_count);
            } else {
                let char_count = input.len() as u16;
                data.push((char_count >> 8) as u8);
                data.push((char_count & 0xFF) as u8);
            }

            // Add the input to the data
            data.append(&mut input.as_bytes().to_vec());

            // Table of required bytes per EC level
            let ec_table: [usize; 10] = [13, 22, 34, 48, 62, 76, 88, 110, 132, 154];

            // Get the total number of bytes required at version level
            let total_bytes = ec_table[version - 1];

            let mut aligned_data: Vec<u8> = vec![];

            // Since byte mode is being used, all bytes are shifted 4 to the left to fill the space, and we don't need to calculate terminator padding
            for byte in 0..data.len() {
                if byte == data.len() - 1 {
                    aligned_data.push((data[byte] & 0xF).checked_shl(4).unwrap_or(0));
                } else {
                    let aligned_byte = (data[byte] & 0xF).checked_shl(4).unwrap_or(0) + (data[byte + 1] >> 4);
                    aligned_data.push(aligned_byte);
                }
            }

            // Add 236 followed by 17 until total capacity is filled as specified
            let padding_byte_count = total_bytes - aligned_data.len();

            for i in 0..padding_byte_count {
                if i % 2 == 0 {
                    aligned_data.push(236);
                } else {
                    aligned_data.push(17);
                }
            }

            let size = (version - 1) * 4 + 21; 

            return QR {
                size: size,
                version: version,
                data: aligned_data,
                payload: vec![],
                image: RawImage::filled_with(0, size, size),
                masked: RawImage::filled_with(0, size, size)
            }
        }

        fn generate_error_correction(&mut self) {
            // TODO: Adapt for versions greater than 2
            // If you think I'm gonna actually implement my own Reed-Solomon algorithm in this, you're kidding yourself
            let blocks_table = vec![(13, 1, 0), (22, 1, 0)];

            let (per_block, group_one, group_two) = blocks_table[self.version - 1];

            if group_one > 1 {
                panic!("QR code too big");
            } else {
                // Create specified number of EC codewords
                let enc = Encoder::new(per_block);

                // Get EC codewords only
                let mut ecc = enc.encode(&self.data).ecc().to_vec();

                self.payload.append(&mut self.data);
                self.payload.append(&mut ecc);
            }
        }
        
        fn create_finder_pattern(&mut self, x: usize, y: usize) {
            // TODO: Flip x and y names
            // Any data inserted is represented as 10/11 instead of 0/1
            // so that masking algorithm knows to skip it
            for k in 0..7 {
                for j in 0..7 {
                    if k == 0 || k == 6 {
                        self.image[(j + x, k + y)] = 11;
                    } else if k == 1 || k == 5 {
                        self.image[(j + x, k + y)] = match j {
                            0 | 6 => 11,
                            _ => 10
                        };
                    } else {
                        self.image[(j + x, k + y)] = match j {
                            1 | 5 => 10,
                            _ => 11
                        };
                    }
                }
            }
        }

        // Helper method to return a bit at offset from a value
        fn get_bit(offset: usize, value: usize) -> usize {
            return (value >> offset) & 1;
        }

        // Places all reserved areas before data is inserted
        fn place_reserved_areas(&mut self) {
            // Add finders
            self.create_finder_pattern(0, 0);
            self.create_finder_pattern(self.size - 7, 0);
            self.create_finder_pattern(0, self.size - 7);

            // Add separators and format information areas
            // Not terribly efficient, but it's clean code
            for y in 0..self.size {
                for x in 0..self.size {
                    // Insert separators 
                    // Only check 1s, since only the edges of the finder patterns need separators
                    if self.image[(y, x)] == 11 {
                        for k in [-1, 1].iter() {
                            for j in [-1, 1].iter() {
                                // Get the adjacent squares
                                let x_offset = (x as isize) + j;
                                let y_offset = (y as isize) + k;

                                // Ignore negative indexes/outside indexes or there's gonna be P R O B L E M S
                                if x_offset >= 0 && y_offset >= 0 && x_offset < (self.size as isize) && y_offset < (self.size as isize) {
                                    let x_i = x_offset as usize;
                                    let y_i = y_offset as usize;
                                   
                                    // If the adjacent square is uninitialized, it needs to be blank
                                    if self.image[(y_i, x_i)] == 3 {
                                        self.image[(y_i, x_i)] = 10;
                                    }
                                }
                            } 
                        }
                    }

                    // Insert format information areas, represented as 2
                    // Some parts will be overwritten later, but that's ok
                    if x == 8 {
                        if y < 9 || y > (self.size - 9) {
                            self.image[(y, x)] = 2;
                        }
                    } else if y == 8 {
                        if x < 9 || x > (self.size - 9) {
                            self.image[(y, x)] = 2;
                        }
                    }
                }
            }

            // Add alignment patterns
            // Version 1 has none
            if self.version > 1 {
                // TODO: Adapt for versions greater than 6
                let start = self.size - 9;
                for y in 0..5 {
                    for x in 0..5 {
                        if y == 0 || y == 4 {
                            self.image[(y + start, x + start)] = 11;
                        } else if y == 1 || y == 3 {
                            self.image[(y + start, x + start)] = match x {
                                0 | 4 => 11,
                                _ => 10
                            }
                        } else {
                            self.image[(y + start, x + start)] = match x {
                                1 | 3 => 10,
                                _ => 11
                            }
                        }
                    }
                }
            }

            // Add vertical timing pattern
            for y in 8..(self.size - 7) {
                if y % 2 == 0 {
                    self.image[(y, 6)] = 11; 
                } else {
                    self.image[(y, 6)] = 10;
                }
            }

            // Add horizontal timing pattern
            for x in 8..(self.size - 7) {
                if x % 2 == 0 {
                    self.image[(6, x)] = 11; 
                } else {
                    self.image[(6, x)] = 10;
                }
            }

            // Add dark module
            self.image[((4 * self.version) + 9, 8)] = 11;
        }

        /*
            Places data into the code
            10/11 represents 0/1 that should not be masked later
            0/1 represents maskable data
            2 represents reversed format areas
            3 represents uninitialized space
        */
        fn place_modules(&mut self) {
            // Fill grid with 3 to represent uninitialized space
            self.image = RawImage::filled_with(3, self.size, self.size);
            self.place_reserved_areas();

            // Place data into the code
            // This took me hours to get working
            let total_bits = self.payload.len() * 8;
            let mut bit_index = 0;

            // Start at the bottom-right corner
            let mut x: isize = self.size as isize - 1;
            let mut y: isize = self.size as isize - 1;

            // Change in y and x to move zig-zag up and down
            let mut y_step: isize = -1;
            let mut x_step: isize = -1; 

            while bit_index < total_bits {
                // If the area is uninitialized, write the next bit of data
                if self.image[(y as usize, x as usize)] == 3 {
                    // The byte is just the bit-index floor division 8
                    // and then the next bit is 7 - (index % 8)
                    // Needed since the data is obviously a vector of bytes, not bits
                    let byte = bit_index / 8;
                    let bit = 7 - (bit_index % 8);

                    let to_write = QR::get_bit(bit, self.payload[byte] as usize);
                    self.image[(y as usize, x as usize)] = to_write as u8;

                    bit_index += 1;
                }

                x += x_step; 

                // Reverse the direction of x every step to zig-zag, and raise y every second placment
                if x_step == -1 {
                    x_step = 1;
                } else {
                    x_step = -1;
                    y += y_step;
                }

                // If we're at the top or bottom and x has placed both elements, reverse the y-step
                if (y == -1 || y == self.size as isize) && x_step == -1 { 
                    if y_step == -1 {
                        y_step = 1;
                        y = 0
                    } else {
                        y_step = -1;
                        y = self.size as isize - 1;
                    }
                
                    // At the vertical timing indicator, we need to skip the column entirely
                    if x == 8 {
                        x = 5;
                    } else {
                        x -= 2;
                    }
                }
            }

            // Any leftover space becomes 0
            for y in 0..self.size {
                for x in 0..self.size {
                    if self.image[(y, x)] == 3 {
                        self.image[(y, x)] = 0;
                    }
                }
            }
        }

        // Helper method to copy an Array2D, since it has no copy trait
        fn copy_image(&self) -> RawImage {
            let size = self.image.column_len();
            let mut new_image = RawImage::filled_with(0, size, size);

            for y in 0..size {
                for x in 0..size {
                    new_image[(y, x)] = self.image[(y, x)];
                }
            }

            return new_image;
        }
        
        // Helper method which flips only 0 to 1 and 1 to 0 and ignores all other values
        fn flip(x: usize, y: usize, image: &mut RawImage) {
            if image[(y, x)] == 1 {
                image[(y, x)] = 0; 
            } else if image[(y, x)] == 0 {
                image[(y, x)] = 1;
            }   
        }
        
        // Inserts the format pattern into the masked array of QR codes
        fn generate_format_pattern(&self, images: &mut Vec<Array2D<u8>>) {
            // Format strings at Q error for each mask pattern
            let format_strings = vec![0x355F, 0x3068, 0x3F31, 0x3A06, 0x24B4, 0x2183, 0x2EDA, 0x2BED];

            for i in 0..8 {
                let format_string = format_strings[i];

                let mut horizontal_bit = 0;
                let mut vertical_bit = 0;

                // Horizontal format information
                for x in 0..self.size {
                    if images[i][(8, x)] == 2 {
                        if x < self.size / 2 {
                            images[i][(8, x)] = QR::get_bit(14 - horizontal_bit, format_string) as u8;
                        } else {
                            // 7 is repeated on the other side
                            images[i][(8, x)] = QR::get_bit(15 - horizontal_bit, format_string) as u8;
                        }

                        horizontal_bit += 1;
                    }
                }

                // Vertical format information
                for y in 0..self.size {
                    if images[i][(y, 8)] == 2 {
                        images[i][(y, 8)] = QR::get_bit(vertical_bit, format_string) as u8;

                        // Skip 7 as it's already placed
                        if vertical_bit == 7 {
                            vertical_bit += 1;
                        }

                        vertical_bit += 1;
                    }
                }
            }
        }

        // Evaluates each mask against the 4 test criteria and returns the index of the best one
        fn evaluate_masks(&self, masked: &Vec<RawImage>) -> usize {
            // Vector to store total penalty for each mask
            let mut penalties = vec![0; 8];

            for (i, mask) in masked.iter().enumerate() {
                // Evaluation 1: Run lengths of same color of 5 or higher
                let mut current_color = 2;
                let mut run_length = 0;

                // Horizontal runs
                for y in 0..self.size {
                    for x in 0..self.size {
                        if current_color != mask[(y, x)] {
                            current_color = mask[(y, x)];
                            
                            if run_length >= 5 {
                                penalties[i] += 3 + (run_length - 5);
                            }
                            
                            run_length = 0;
                        } 
                        
                        run_length += 1;
                    }

                    // Write any remaining run length penalties that happen on the edge
                    if run_length >= 5 {
                        penalties[i] += 3 + (run_length - 5);
                    }

                    current_color = 2;
                    run_length = 0;
                }

                // Vertical runs
                for x in 0..self.size {
                    for y in 0..self.size {
                        if current_color != mask[(y, x)] {
                            current_color = mask[(y, x)];
                            
                            if run_length >= 5 {
                                penalties[i] += 3 + (run_length - 5);
                            }
                            
                            run_length = 0;
                        } 
                        
                        run_length += 1;
                    }

                    // Write any remaining run length penalties that happen on the edge
                    if run_length >= 5 {
                        penalties[i] += 3 + (run_length - 5);
                    }

                    current_color = 2;
                    run_length = 0;
                }

                // Evaluation 2: 2x2 blocks of the same color
                for y in 0..(self.size - 1) {
                    for x in 0..(self.size - 1) {
                        let square = vec![mask[(y, x)], mask[(y + 1, x)], mask[(y, x + 1)], mask[(y + 1, x + 1)]];

                        // If square contains all same color
                        if square.iter().all(|&item| item == mask[(y, x)]) {
                            penalties[i] += 3;
                        }
                    }
                }

                // Evaluation 3: Check for a specific pattern appearing either horizontally or vertically
                // Horizontal
                for y in 0..self.size {
                    for x in 0..(self.size - 10) {
                        let pattern_a = vec![1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0];
                        let pattern_b = vec![0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1]; 

                        let test_pattern = (0..11).map(|i| mask[(y, x + i)]).collect::<Vec<u8>>();

                        if test_pattern == pattern_a || test_pattern == pattern_b {
                            penalties[i] += 40;
                        }
                    }
                }

                // Vertical
                for x in 0..self.size {
                    for y in 0..(self.size - 10) {
                        let pattern_a = vec![1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0];
                        let pattern_b = vec![0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1]; 

                        let test_pattern = (0..11).map(|i| mask[(y + i, x)]).collect::<Vec<u8>>();

                        if test_pattern == pattern_a || test_pattern == pattern_b {
                            penalties[i] += 40;
                        }
                    }
                }

                // Evaluation 4: Ratio of dark to light modules
                let total_modules = self.size * self.size;
                let mut dark_modules: isize = 0;

                for row in mask.rows_iter() {
                    for module in row {
                        dark_modules += *module as isize;
                    }
                }

                let percentage_dark = ((dark_modules as f32 / total_modules as f32) * 100.0) as isize;
                let previous_multiple = percentage_dark - (percentage_dark % 5);
                let next_multiple = percentage_dark + (5 - (percentage_dark % 5));

                let multiples = vec![previous_multiple, next_multiple];
                let subtracted = multiples.iter().map(|&x| isize::abs(x - 50)).collect::<Vec<_>>();
                let divided = subtracted.iter().map(|&x| x / 5).collect::<Vec<_>>();
                penalties[i] += divided.into_iter().min().unwrap() * 10;
            }

            // The best mask is the one with the lowest score
            let best_code_index: usize = penalties
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(Ordering::Equal))
                .map(|(index, _)| index).unwrap();
            
            println!("Best mask is mask {} with penalty {}", best_code_index, penalties[best_code_index]);

            return best_code_index;
        }

        // Masks the QR code, evaluating each version. Also inserts the format pattern
        fn mask_and_format(&mut self) {
            // Generate 8 maskable copies to evaluate each mask
            let mut masked = (0..8).map(|_| self.copy_image()).collect::<Vec<RawImage>>();

            // Generate the 8 different masks, and revert 10/11 to 0/1
            for y in 0..self.size {
                for x in 0..self.size {
                    // Mask 0
                    if (x + y) % 2 == 0 {
                        QR::flip(x, y, &mut masked[0])
                    }

                    // Mask 1
                    if y % 2 == 0 {
                        QR::flip(x, y, &mut masked[1]);
                    }

                    // Mask 2
                    if x % 3 == 0 {
                        QR::flip(x, y, &mut masked[2]);
                    }

                    // Mask 3
                    if (x + y) % 3 == 0 {
                        QR::flip(x, y, &mut masked[3]);
                    }

                    // Mask 4
                    if ((y / 2) + (x / 3)) % 2 == 0 {
                        QR::flip(x, y, &mut masked[4]);
                    }

                    // Mask 5
                    if ((x * y) % 2) + ((x * y) % 3) == 0 {
                        QR::flip(x, y, &mut masked[5]);
                    }

                    // Mask 6
                    if (((x * y) % 2) + ((x * y) % 3)) % 2 == 0 {
                        QR::flip(x, y, &mut masked[6]);
                    }

                    // Mask 7
                    if (((x + y) % 2) + ((x * y) % 3)) % 2 == 0 {
                        QR::flip(x, y, &mut masked[7]);
                    }

                    // Revert 10/11 to 0/1 for each mask
                    for i in 0..8 {
                        if masked[i][(y, x)] == 10 {
                            masked[i][(y, x)] = 0;
                        } else if masked[i][(y, x)] == 11 {
                            masked[i][(y, x)] = 1;
                        }
                    }
                }
            }

            // Format patterns have to be inserted now, as they are part of the mask evaluation
            self.generate_format_pattern(&mut masked);

            // Evaluate all the masks
            let best = self.evaluate_masks(&masked);
            
            // Copy the best mask to self.masked
            for y in 0..self.size {
                for x in 0..self.size {
                    self.masked[(y, x)] = masked[best][(y, x)];
                }
            }
        }

        // Prints the QR code to terminal
        fn print_qr(&self, image: &RawImage) {
            let gap = "  ".repeat((self.size + 4) * 2);

            println!("{}", gap);
            println!("{}", gap);

            for row_iter in image.rows_iter() {
                print!("    ");

                for module in row_iter {
                    if *module == 1 || *module == 11 {
                        print!("██");
                    } else if *module == 3 {
                        print!("..");
                    } else if *module == 2 {
                        print!("FF");
                    } else {
                        print!("  ");
                    }
                }

                println!("    ");
            }

            println!("{}", gap);
            println!("{}", gap);
        }

        pub fn generate(&mut self) {
            self.generate_error_correction();
            self.place_modules();
            self.mask_and_format();
            self.print_qr(&self.masked);
        }

        pub fn save_image(&self, path: String, size: u32) {
            // Add quiet zone of 4 pixels around the code
            let mut imgbuf = image::GrayImage::new(self.size as u32 + 8, self.size as u32 + 8);

            for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
                // Only write from the code if we're in range of the code or else we're gonna overrun
                // and there will be P R O B L E M S
                if x > 3 && (x as usize) < self.size + 4 && y > 3 && (y as usize) < self.size + 4 { 
                    *pixel = image::Luma([(1 - self.masked[(y as usize - 4, x as usize - 4)]) * 255]);
                } else {
                    *pixel = image::Luma([255u8]);
                }
            }

            // Resize the image since 30x30 pixel images are apparently "not high enough resolution" now
            // Use nearest-neighbor so it actually looks good
            let resized = image::imageops::resize(&imgbuf, size, size, image::imageops::FilterType::Nearest);
            resized.save(&path).unwrap();
            println!("Saved to {}", path);
        }
    }
}