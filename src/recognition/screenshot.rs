use crate::errors::KickbotError;
use crate::errors::KickbotError::ScreenshotError;
use opencv::core::{
    AlgorithmHint, Mat, MatTrait, MatTraitConst, Rect, Vector, CV_32FC4, CV_8UC3, CV_8UC4,
};
use opencv::imgcodecs::IMWRITE_JPEG_QUALITY;
use opencv::imgproc::{cvt_color, COLOR_BGRA2BGR};
use opencv::prelude::*;
use opencv::{self as cv, highgui};
use serenity::futures::StreamExt;
use win_screenshot::capture::capture_window;
use win_screenshot::prelude::find_window;

#[derive(Clone)]
pub struct Screenshot {
    pub(crate) image: Mat,
    pub(crate) height: i32,
    pub(crate) width: i32,
}

impl Screenshot {
    pub fn take_screenshot() -> Result<Screenshot, KickbotError> {
        let hwnd = find_window("Battlefield™ 1")
            .map_err(|err| ScreenshotError("Couldn't find Battlefield 1 window".to_string()))?;
        let buf = capture_window(hwnd)
            .map_err(|err| ScreenshotError("Error taking screenshot".to_string()))?;

        let mut mat = unsafe {
            Mat::new_nd_vec(
                &Vector::from_slice(&[buf.height as i32, buf.width as i32]),
                CV_8UC4,
            )?
        };

        mat.data_bytes_mut()?.copy_from_slice(buf.pixels.as_slice());

        Ok(Screenshot {
            image: mat,
            width: buf.width as i32,
            height: buf.height as i32,
        })
    }

    pub fn from(image: &Mat) -> Screenshot {
        Screenshot {
            image: image.clone(),
            width: image.cols(),
            height: image.rows(),
        }
    }

    pub fn crop_image(&self, box2d: Rect) -> Result<Self, KickbotError> {
        Ok(Screenshot::from(&self.image.roi(box2d)?.clone_pointee()))
    }

    pub fn display(&self) -> Result<&Self, KickbotError> {
        let window_name = "window";
        let _ = highgui::named_window(window_name, highgui::WINDOW_AUTOSIZE)?;

        loop {
            highgui::imshow(window_name, &self.image)?;
            highgui::wait_key(0)?;
            break;
        }
        Ok(self)
    }

    fn sanitize_filename(name: &str) -> String {
        name.trim()
            .chars()
            .filter(|c| {
                !c.is_control() && // Removes \n, \r, \t, etc.
                    !['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '].contains(c)
            })
            .map(|c| c) // Keep the valid chars
            .collect::<String>()
    }

    pub fn save(&self, filename: &str) -> Result<(), KickbotError> {
        let safe_name = Self::sanitize_filename(filename);
        let path = format!("screenshots/{}.jpg", safe_name);
        let mut params = Vector::default();
        params.push(IMWRITE_JPEG_QUALITY);
        params.push(90);
        if !cv::imgcodecs::imwrite(path.as_str(), &self.image, &params)? {
            return Err(ScreenshotError(format!("Error saving file {}", path)));
        }
        println!("Saved");
        Ok(())
    }
}
