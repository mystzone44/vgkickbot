use crate::recognition::screenshot::Screenshot;
use opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT;
use opencv::core::{no_array, Mat, MatTrait, MatTraitConst, Size, Vector};
use opencv::imgproc::{CLAHETrait, COLOR_BGR2GRAY, COLOR_BGR2HSV, COLOR_RGB2HSV};
use opencv::{self as cv};

#[derive(Copy, Clone, Debug)]
pub struct RGB {
    pub(crate) r: i16,
    pub(crate) g: i16,
    pub(crate) b: i16,
}

fn clamp<T: Ord>(num: T, min: T, max: T) -> T {
    std::cmp::min(std::cmp::max(num, min), max)
}

pub fn enhance_image(
    image: &Screenshot,
    ally_colour: RGB,
    enemy_colour: RGB,
) -> Result<(Mat, Mat), cv::Error> {
    let target_height = 100f64;
    let ratio = target_height / image.height as f64;
    let size = Size::new(
        f64::trunc(ratio * image.width as f64) as i32,
        f64::trunc(ratio * image.height as f64) as i32,
    );
    let mut resized_image = Mat::default();
    cv::imgproc::resize(
        &image.image,
        &mut resized_image,
        size,
        0f64,
        0f64,
        cv::imgproc::INTER_CUBIC,
    )?;

    let mut image_hsv = Mat::default();
    cv::imgproc::cvt_color(
        &resized_image,
        &mut image_hsv,
        COLOR_BGR2HSV,
        0,
        ALGO_HINT_DEFAULT,
    )?;

    let threshold = 80;
    let mut masks: Vector<Mat> = Vector::new();
    for colour in [ally_colour, enemy_colour] {
        let mut rgb = Mat::new_rows_cols_with_default(
            1,
            1,
            cv::core::CV_8UC3,
            cv::core::Scalar::new(colour.r as f64, colour.b as f64, colour.g as f64, 0.0),
        )?;
        let mut hsv = rgb.clone();

        cv::imgproc::cvt_color(&rgb, &mut hsv, COLOR_RGB2HSV, 0, ALGO_HINT_DEFAULT)?;
        let h = hsv.at::<cv::core::Vec3b>(0)?[0] as i16;

        let h_min = clamp(h - threshold, 0, 180) as u8;
        let h_max = clamp(h + threshold, 0, 180) as u8;
        let lower = Vector::from_slice(&[h_min, 110, 100]);
        let upper = Vector::from_slice(&[h_max, 255, 255]);

        let mut range_output = Mat::default();
        cv::core::in_range(&image_hsv, &lower, &upper, &mut range_output)?;
        masks.push(range_output);
    }

    let mut mask = Mat::default();
    cv::core::bitwise_or(
        &masks.get(0).unwrap(),
        &masks.get(1).unwrap(),
        &mut mask,
        &no_array(),
    )?;
    cv::core::bitwise_not(&mask.clone(), &mut mask, &no_array())?;
    let white_background = image_hsv.clone().set_to(&[255], &no_array())?;
    let mut result = Mat::default();
    cv::core::bitwise_and(&white_background, &white_background, &mut result, &mask)?;

    Ok((result, mask))
}

pub fn enhance_weapon_image(image: &Mat) -> Result<Mat, cv::Error> {
    let mut image_grey = Mat::default();
    cv::imgproc::cvt_color(
        &image,
        &mut image_grey,
        COLOR_BGR2GRAY,
        0,
        ALGO_HINT_DEFAULT,
    )?;

    let mut result = Mat::default();
    cv::imgproc::create_clahe(1.0, Size::new(4, 4))?.apply(&image_grey, &mut result)?;

    Ok(result)
}
