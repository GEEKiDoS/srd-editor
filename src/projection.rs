use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionError(pub String);

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProjectionError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix4x4 {
    pub rows: [[f32; 4]; 4],
}

pub fn identity_matrix4x4_game() -> Matrix4x4 {
    Matrix4x4 {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

pub fn viewport_matrix_game(width: i32, height: i32) -> Matrix4x4 {
    let mut result = identity_matrix4x4_game();
    let half_width = (width as f32) * 0.5;
    let half_height = (height as f32) * 0.5;
    result.rows[0][0] = half_width;
    result.rows[0][3] = half_width;
    result.rows[1][1] = (height as f32) * -0.5;
    result.rows[1][3] = half_height;
    result
}

pub fn mul_matrix4x4_game(lhs: &Matrix4x4, rhs: &Matrix4x4) -> Matrix4x4 {
    let mut result = Matrix4x4 {
        rows: [[0.0; 4]; 4],
    };

    for (result_row, lhs_row) in result.rows.iter_mut().zip(lhs.rows) {
        for (column, value) in result_row.iter_mut().enumerate() {
            // sub_604010 performs these four SIMD products and three additions in
            // this exact grouping for every output row.
            let w_times_row_3 = lhs_row[3] * rhs.rows[3][column];
            let z_times_row_2 = lhs_row[2] * rhs.rows[2][column];
            let wz = w_times_row_3 + z_times_row_2;
            let y_times_row_1 = lhs_row[1] * rhs.rows[1][column];
            let x_times_row_0 = lhs_row[0] * rhs.rows[0][column];
            let yx = y_times_row_1 + x_times_row_0;
            *value = wz + yx;
        }
    }

    result
}

/// Reproduces `ceylon_inverse_matrix4x4` (`0x6B4170`). The game performs a
/// scaled partial-pivot LU decomposition through four swappable row pointers,
/// then solves the four identity columns. A zero source row or zero pivot is
/// not reported as an error: the input matrix is copied to the output.
pub fn inverse_matrix4x4_game(matrix: &Matrix4x4) -> Matrix4x4 {
    let mut rows = matrix.rows;
    let mut row_order = [0usize, 1, 2, 3];
    let mut row_scale = [0.0f32; 4];

    for row in 0..4 {
        let mut largest = rows[row][0].abs();
        largest = largest.max(rows[row][1].abs());
        largest = largest.max(rows[row][2].abs());
        largest = largest.max(rows[row][3].abs());
        if largest == 0.0 {
            return *matrix;
        }
        row_scale[row] = 1.0 / largest;
    }

    for column in 0..4 {
        let mut pivot_slot = column;
        let mut pivot_score =
            (rows[row_order[column]][column] * row_scale[row_order[column]]).abs();
        for slot in (column + 1)..4 {
            let score = (rows[row_order[slot]][column] * row_scale[row_order[slot]]).abs();
            if score > pivot_score {
                pivot_score = score;
                pivot_slot = slot;
            }
        }
        row_order.swap(column, pivot_slot);

        let pivot_row = row_order[column];
        let pivot = rows[pivot_row][column];
        if pivot == 0.0 {
            return *matrix;
        }

        for slot in (column + 1)..4 {
            let row = row_order[slot];
            rows[row][column] = rows[row][column] / pivot;
            let multiplier = rows[row][column];
            for trailing_column in (column + 1)..4 {
                rows[row][trailing_column] =
                    rows[row][trailing_column] - (rows[pivot_row][trailing_column] * multiplier);
            }
        }
    }

    let mut inverse = Matrix4x4 {
        rows: [[0.0; 4]; 4],
    };
    for output_column in 0..4 {
        let mut solved = [0.0f32; 4];
        for logical_row in 0..4 {
            solved[logical_row] = if row_order[logical_row] == output_column {
                1.0
            } else {
                0.0
            };
        }

        for logical_row in 0..4 {
            let value = solved[logical_row];
            for following_row in (logical_row + 1)..4 {
                solved[following_row] =
                    solved[following_row] - (rows[row_order[following_row]][logical_row] * value);
            }
        }

        for logical_row in (0..4).rev() {
            let mut value = solved[logical_row];
            for following_column in (logical_row + 1)..4 {
                value = value
                    - (rows[row_order[logical_row]][following_column] * solved[following_column]);
            }
            solved[logical_row] = value / rows[row_order[logical_row]][logical_row];
        }

        for output_row in 0..4 {
            inverse.rows[output_row][output_column] = solved[output_row];
        }
    }
    inverse
}

pub fn compose_screen_matrix_game(
    width: i32,
    height: i32,
    pre_viewport_transform: &Matrix4x4,
    renderer_transform: &Matrix4x4,
) -> Matrix4x4 {
    let viewport = viewport_matrix_game(width, height);
    let viewport_pre_transform = mul_matrix4x4_game(&viewport, pre_viewport_transform);
    mul_matrix4x4_game(&viewport_pre_transform, renderer_transform)
}

pub fn project_cast_corners_to_screen(
    world_corners: &[[f32; 3]; 4],
    is_2d: bool,
    screen_matrix: Option<&Matrix4x4>,
) -> Result<[[f32; 2]; 4], ProjectionError> {
    if is_2d {
        return Ok(world_corners.map(|point| [point[0], point[1]]));
    }
    let screen_matrix = screen_matrix
        .ok_or_else(|| ProjectionError("3D CAST screen projection needs a 4x4 matrix".into()))?;
    Ok(world_corners.map(|point| project_point_to_screen_game(point, screen_matrix)))
}

/// Reproduces `sub_AC6660 -> sub_1049940` for the renderer visibility test
/// when `SrRenderer+0x24C` has been initialized from a resolved render target.
/// The target rectangle is `[width/2, height/2, width/2, height/2]`; the CAST
/// rectangle is built from the four projected corners as center/half extents.
pub fn cast_overlaps_render_target_game(
    world_corners: &[[f32; 3]; 4],
    is_2d: bool,
    screen_matrix: Option<&Matrix4x4>,
    width: i32,
    height: i32,
) -> Result<bool, ProjectionError> {
    let projected = project_cast_corners_to_screen(world_corners, is_2d, screen_matrix)?;
    let [minimum_x, maximum_x] = sse_bounds(projected.map(|point| point[0]));
    let [minimum_y, maximum_y] = sse_bounds(projected.map(|point| point[1]));

    let half = 0.5f32;
    let cast_rectangle = [
        (minimum_x - maximum_x) * half + maximum_x,
        (minimum_y - maximum_y) * half + maximum_y,
        (maximum_x - minimum_x) * half,
        (maximum_y - minimum_y) * half,
    ];
    let target_half_width = (width as f32) * half;
    let target_half_height = (height as f32) * half;
    let target_rectangle = [
        target_half_width,
        target_half_height,
        target_half_width,
        target_half_height,
    ];
    Ok(center_half_rectangles_overlap_game(
        target_rectangle,
        cast_rectangle,
    ))
}

fn sse_bounds(values: [f32; 4]) -> [f32; 2] {
    let mut minimum = 9_999_999.0f32;
    let mut maximum = -9_999_999.0f32;
    for value in values {
        minimum = sse_min(value, minimum);
        maximum = sse_max(value, maximum);
    }
    [minimum, maximum]
}

/// MINSS/MAXSS return the second operand for unordered and equal inputs.
/// Operand order is kept identical to `sub_AC6660`, preserving signed zero
/// and the binary's NaN behavior.
fn sse_min(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() || left >= right {
        right
    } else {
        left
    }
}

fn sse_max(left: f32, right: f32) -> f32 {
    if left.is_nan() || right.is_nan() || left <= right {
        right
    } else {
        left
    }
}

fn center_half_rectangles_overlap_game(left: [f32; 4], right: [f32; 4]) -> bool {
    let mut delta_x = left[0] - right[0];
    if delta_x < 0.0 {
        delta_x = -delta_x;
    }
    if delta_x > left[2] + right[2] {
        return false;
    }

    let mut delta_y = left[1] - right[1];
    if delta_y < 0.0 {
        delta_y = -delta_y;
    }
    // COMISS + SETBE treats unordered inputs as overlap. Writing this as the
    // negation of `>` reproduces that behavior, unlike Rust's direct `<=`.
    !(delta_y > left[3] + right[3])
}

pub fn project_point_to_screen_game(point: [f32; 3], screen_matrix: &Matrix4x4) -> [f32; 2] {
    let [x, y, z] = point;
    let rows = &screen_matrix.rows;

    let x_times_0 = x * rows[0][0];
    let mut projected_x = y * rows[0][1];
    projected_x += x_times_0;
    let z_times_0 = z * rows[0][2];
    projected_x += z_times_0;
    projected_x += rows[0][3];

    let mut projected_y = x * rows[1][0];
    let y_times_1 = y * rows[1][1];
    projected_y += y_times_1;
    let z_times_1 = z * rows[1][2];
    projected_y += z_times_1;
    projected_y += rows[1][3];

    let mut projected_w = x * rows[3][0];
    let y_times_3 = y * rows[3][1];
    projected_w += y_times_3;
    let z_times_3 = z * rows[3][2];
    projected_w += z_times_3;
    projected_w += rows[3][3];

    let reciprocal_w = 1.0f32 / projected_w;
    [projected_x * reciprocal_w, projected_y * reciprocal_w]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_dimensional_casts_copy_xy_without_a_matrix() {
        let corners = [
            [1.0, 2.0, 3.0],
            [4.0, 5.0, 6.0],
            [7.0, 8.0, 9.0],
            [10.0, 11.0, 12.0],
        ];
        assert_eq!(
            project_cast_corners_to_screen(&corners, true, None).unwrap(),
            [[1.0, 2.0], [4.0, 5.0], [7.0, 8.0], [10.0, 11.0]]
        );
    }

    #[test]
    fn three_dimensional_casts_use_rows_zero_one_and_three_then_divide_by_w() {
        let matrix = Matrix4x4 {
            rows: [
                [2.0, 0.0, 0.0, 0.0],
                [0.0, 4.0, 0.0, 0.0],
                [99.0, 99.0, 99.0, 99.0],
                [0.0, 0.0, 0.0, 2.0],
            ],
        };
        assert_eq!(
            project_point_to_screen_game([3.0, 5.0, 7.0], &matrix),
            [3.0, 10.0]
        );
        assert!(project_cast_corners_to_screen(&[[0.0; 3]; 4], false, None).is_err());
    }

    #[test]
    fn cast_visibility_uses_inclusive_center_half_aabb_overlap() {
        let touching = [
            [100.0, 10.0, 0.0],
            [110.0, 10.0, 0.0],
            [100.0, 20.0, 0.0],
            [110.0, 20.0, 0.0],
        ];
        assert!(cast_overlaps_render_target_game(&touching, true, None, 100, 100).unwrap());

        let outside = touching.map(|mut point| {
            point[0] += 11.0;
            point
        });
        assert!(!cast_overlaps_render_target_game(&outside, true, None, 100, 100).unwrap());
    }

    #[test]
    fn cast_visibility_projects_3d_corners_through_renderer_screen_matrix() {
        let corners = [
            [-1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
        ];
        let screen = viewport_matrix_game(1920, 1080);
        assert!(
            cast_overlaps_render_target_game(&corners, false, Some(&screen), 1920, 1080,).unwrap()
        );
    }

    #[test]
    fn cast_visibility_preserves_unordered_comiss_overlap_behavior() {
        assert!(center_half_rectangles_overlap_game(
            [0.0, 0.0, 1.0, 1.0],
            [f32::NAN, f32::NAN, 1.0, 1.0],
        ));
    }

    #[test]
    fn viewport_matrix_maps_ndc_to_d3d9_screen_coordinates() {
        let viewport = viewport_matrix_game(1280, 720);
        assert_eq!(
            project_point_to_screen_game([-1.0, 1.0, 0.0], &viewport),
            [0.0, 0.0]
        );
        assert_eq!(
            project_point_to_screen_game([1.0, -1.0, 0.0], &viewport),
            [1280.0, 720.0]
        );
    }

    #[test]
    fn four_by_four_product_uses_the_game_operand_order() {
        let lhs = Matrix4x4 {
            rows: [
                [1.0, 2.0, 3.0, 4.0],
                [5.0, 6.0, 7.0, 8.0],
                [9.0, 10.0, 11.0, 12.0],
                [13.0, 14.0, 15.0, 16.0],
            ],
        };
        let rhs = Matrix4x4 {
            rows: [
                [17.0, 18.0, 19.0, 20.0],
                [21.0, 22.0, 23.0, 24.0],
                [25.0, 26.0, 27.0, 28.0],
                [29.0, 30.0, 31.0, 32.0],
            ],
        };
        assert_eq!(
            mul_matrix4x4_game(&lhs, &rhs).rows,
            [
                [250.0, 260.0, 270.0, 280.0],
                [618.0, 644.0, 670.0, 696.0],
                [986.0, 1028.0, 1070.0, 1112.0],
                [1354.0, 1412.0, 1470.0, 1528.0],
            ]
        );
    }

    #[test]
    fn inverse_identity_is_identity() {
        let identity = identity_matrix4x4_game();
        assert_eq!(inverse_matrix4x4_game(&identity), identity);
    }

    #[test]
    fn inverse_round_trip_matches_identity() {
        let matrix = Matrix4x4 {
            rows: [
                [2.0, 0.0, 0.0, 4.0],
                [0.0, 3.0, 0.0, -6.0],
                [0.0, 0.0, 5.0, 10.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let inverse = inverse_matrix4x4_game(&matrix);
        let product = mul_matrix4x4_game(&matrix, &inverse);
        for row in 0..4 {
            for column in 0..4 {
                let expected = if row == column { 1.0 } else { 0.0 };
                assert!((product.rows[row][column] - expected).abs() < 0.000001);
            }
        }
    }

    #[test]
    fn inverse_singular_matrix_copies_the_input() {
        let singular = Matrix4x4 {
            rows: [
                [1.0, 2.0, 3.0, 4.0],
                [0.0, 0.0, 0.0, 0.0],
                [5.0, 6.0, 7.0, 8.0],
                [9.0, 10.0, 11.0, 12.0],
            ],
        };
        assert_eq!(inverse_matrix4x4_game(&singular), singular);
    }

    #[test]
    fn screen_composition_matches_the_two_binary_multiplications() {
        let identity = identity_matrix4x4_game();
        assert_eq!(
            compose_screen_matrix_game(640, 480, &identity, &identity),
            viewport_matrix_game(640, 480)
        );
    }
}
