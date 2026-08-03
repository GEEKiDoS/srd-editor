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
    fn screen_composition_matches_the_two_binary_multiplications() {
        let identity = identity_matrix4x4_game();
        assert_eq!(
            compose_screen_matrix_game(640, 480, &identity, &identity),
            viewport_matrix_game(640, 480)
        );
    }
}
