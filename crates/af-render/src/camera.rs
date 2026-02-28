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
    /// Applique les transformations de la caméra de la config actuelle sur l'`input` pour générer `output`.
    /// `input` et `output` doivent avoir un espace de buffer pré-alloué de taille finale (généralement `canvas_width` x `canvas_height`).
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn apply_transform(config: &RenderConfig, input: &FrameBuffer, output: &mut FrameBuffer) {
        let zoom = config.camera_zoom_amplitude.max(0.01);
        let rot = config.camera_rotation;
        let pan_x = config.camera_pan_x;
        let pan_y = config.camera_pan_y;

        // Si la caméra est totalement neutre intrinsèquement (identique à l'Identity Matrix),
        // On pourrait juste memcopy ou retourner l'input pour sauver ~1ms.
        // Mais comme c'est fluide et audio-réactif, elle est rarement exactement neutre.
        let is_identity = (zoom - 1.0).abs() < f32::EPSILON
            && rot.abs() < f32::EPSILON
            && pan_x.abs() < f32::EPSILON
            && pan_y.abs() < f32::EPSILON
            && input.width == output.width
            && input.height == output.height;

        if is_identity || input.is_camera_baked {
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

        // Stride is 4 bytes (RGBA)
        let out_stride = (output.width * 4) as usize;
        let in_stride = (input.width * 4) as usize;

        // On bind les slices purement localement pour bypasser l'alias `&mut self` dans la closure par-chunk
        let in_data = &input.data;
        let in_width = input.width as i32;
        let in_height = input.height as i32;

        // Pixel-perfect parallel chunks over output rows
        output
            .data
            .par_chunks_exact_mut(out_stride)
            .enumerate()
            .for_each(|(y_out, row)| {
                let y_f = y_out as f32 - center_y;

                for x_out in 0..output.width {
                    let x_f = x_out as f32 - center_x;

                    // Reverse Pan
                    let x_panned = x_f - (pan_x * out_w);
                    let y_panned = y_f - (pan_y * out_h);

                    // Reverse Zoom
                    let x_zoomed = x_panned / zoom;
                    let y_zoomed = y_panned / zoom;

                    // Reverse Rotation
                    let x_src_f = x_zoomed * cos_a - y_zoomed * sin_a + in_center_x;
                    let y_src_f = x_zoomed * sin_a + y_zoomed * cos_a + in_center_y;

                    // Nearest neighbour rounding
                    let x_src = x_src_f.round() as i32;
                    let y_src = y_src_f.round() as i32;

                    let out_idx = (x_out * 4) as usize;

                    // Bounds checking
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
            });
    }
}
