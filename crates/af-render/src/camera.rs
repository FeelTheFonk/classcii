use af_core::config::RenderConfig;
use af_core::frame::FrameBuffer;
use rayon::prelude::*;

/// Système de Caméra Virtuelle effectuant des transformations affines
/// (Zoom, Pan, Rotation) sur les FrameBuffers pixel bruts.
///
/// Fonctionnement R1-Strict Zero-Alloc :
/// La caméra utilise une technique de "Reverse Mapping" (Interpolation au plus proche voisin pour la vitesse)
/// où pour chaque pixel du buffer `output`, on calcule sa position d'origine dans le buffer `input`
/// via la matrice affine inverse.
pub struct VirtualCamera;

impl VirtualCamera {
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    pub fn apply_transform(config: &RenderConfig, input: &FrameBuffer, output: &mut FrameBuffer) {
        let zoom = config.camera_zoom_amplitude.max(0.01);
        let rot = config.camera_rotation % std::f32::consts::TAU;
        let pan_x = config.camera_pan_x;
        let pan_y = config.camera_pan_y;
        let tilt = config.camera_tilt_x;

        let is_identity = (zoom - 1.0).abs() < f32::EPSILON
            && rot.abs() < f32::EPSILON
            && pan_x.abs() < f32::EPSILON
            && pan_y.abs() < f32::EPSILON
            && tilt.abs() < f32::EPSILON
            && input.width == output.width
            && input.height == output.height;

        if is_identity {
            output.data.copy_from_slice(&input.data);
            return;
        }

        let out_w = output.width as f32;
        let out_h = output.height as f32;
        let in_w = input.width as f32;
        let in_h = input.height as f32;

        let center_x = out_w / 2.0;
        let center_y = out_h / 2.0;

        let in_center_x = in_w / 2.0;
        let in_center_y = in_h / 2.0;

        let cos_a = rot.cos();
        let sin_a = rot.sin();

        // Perspective coefficient: maps tilt [-1,1] to a projective warp factor.
        // The denominator `1 + h * y_norm` creates vanishing-point perspective.
        // Clamped to avoid singularities (denominator never reaches 0).
        let h = tilt * 0.8; // scale factor — 0.8 keeps max warp safe

        let out_stride = (output.width * 4) as usize;
        let in_stride = (input.width * 4) as usize;

        let in_data = &input.data;
        let in_width = input.width as i32;
        let in_height = input.height as i32;

        let pixel_count = output.width * output.height;
        let process_row = |y_out: usize, row: &mut [u8]| {
            let y_f = y_out as f32 - center_y;
            // Normalized y for perspective: [-1, 1]
            let y_norm = y_f / center_y.max(1.0);

            for x_out in 0..output.width {
                let x_f = x_out as f32 - center_x;

                // Perspective division: simulate 3D tilt by warping based on vertical position
                let denom = (1.0 + h * y_norm).max(0.1);
                let x_persp = x_f / denom;
                let y_persp = y_f / denom;

                // Reverse Pan
                let x_panned = x_persp - (pan_x * out_w);
                let y_panned = y_persp - (pan_y * out_h);

                // Reverse Zoom
                let x_zoomed = x_panned / zoom;
                let y_zoomed = y_panned / zoom;

                // Reverse Rotation
                let x_src_f = x_zoomed * cos_a - y_zoomed * sin_a + in_center_x;
                let y_src_f = x_zoomed * sin_a + y_zoomed * cos_a + in_center_y;

                let out_idx = (x_out * 4) as usize;

                // Bilinear interpolation
                let x0 = x_src_f.floor() as i32;
                let y0 = y_src_f.floor() as i32;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                if x0 >= 0 && x1 < in_width && y0 >= 0 && y1 < in_height {
                    let fx = x_src_f - x0 as f32;
                    let fy = y_src_f - y0 as f32;
                    let ifx = 1.0 - fx;
                    let ify = 1.0 - fy;

                    let w00 = ifx * ify;
                    let w10 = fx * ify;
                    let w01 = ifx * fy;
                    let w11 = fx * fy;

                    let i00 = (y0 as usize * in_stride) + (x0 as usize * 4);
                    let i10 = i00 + 4;
                    let i01 = i00 + in_stride;
                    let i11 = i01 + 4;

                    for c in 0..4 {
                        row[out_idx + c] = (f32::from(in_data[i00 + c]) * w00
                            + f32::from(in_data[i10 + c]) * w10
                            + f32::from(in_data[i01 + c]) * w01
                            + f32::from(in_data[i11 + c]) * w11)
                            as u8;
                    }
                    continue;
                }

                // Edge fallback: nearest neighbor for border pixels
                let x_src = x_src_f.round() as i32;
                let y_src = y_src_f.round() as i32;
                if x_src >= 0 && x_src < in_width && y_src >= 0 && y_src < in_height {
                    let in_idx = (y_src as usize * in_stride) + (x_src as usize * 4);
                    if in_idx + 3 < in_data.len() {
                        row[out_idx] = in_data[in_idx];
                        row[out_idx + 1] = in_data[in_idx + 1];
                        row[out_idx + 2] = in_data[in_idx + 2];
                        row[out_idx + 3] = in_data[in_idx + 3];
                        continue;
                    }
                }

                // Out-of-bounds -> Black transparent
                row[out_idx] = 0;
                row[out_idx + 1] = 0;
                row[out_idx + 2] = 0;
                row[out_idx + 3] = 0;
            }
        };

        if pixel_count >= 50_000 {
            output
                .data
                .par_chunks_exact_mut(out_stride)
                .enumerate()
                .for_each(|(y, row)| process_row(y, row));
        } else {
            output
                .data
                .chunks_exact_mut(out_stride)
                .enumerate()
                .for_each(|(y, row)| process_row(y, row));
        }
    }
}
