use crate::errors::KickbotError;
use crate::recognition::screenshot::Screenshot;
use opencv::core::{Mat, MatTraitConst, MatTraitConstManual, ToOutputArray, Vector, VectorToVec};
use opencv::imgcodecs::imencode;
use tesseract::{PageSegMode, Tesseract};

pub struct OCR {
    tesseract: Tesseract,
}

impl OCR {
    pub fn new() -> OCR {
        let mut tesseract = Tesseract::new(Some("tessdata"), Some("bf1")).unwrap();
        tesseract.set_page_seg_mode(PageSegMode::PsmSingleLine);
        let tesseract = tesseract
            .set_variable("debug_file", "nul")
            .expect("Couldn't set tesseract debug_file to nul");
        OCR { tesseract }
    }

    fn recognise(mut self, mat: &Mat) -> Result<Self, KickbotError> {
        let mut buf = Vector::new();
        imencode(".png", &mat, &mut buf, &Vector::new())?;
        let data = buf.to_vec();

        self.tesseract = self
            .tesseract
            .set_image_from_mem(data.as_slice())
            .map_err(|err| KickbotError::TesseractError(err.to_string()))?;

        self.tesseract = self
            .tesseract
            .recognize()
            .map_err(|err| KickbotError::TesseractError(err.to_string()))?;
        Ok(self)
    }

    pub fn recognise_from_mat(mut self, image: &Mat) -> Result<Self, KickbotError> {
        self.recognise(image)
    }

    pub fn recognise_from_screenshot(mut self, image: &Screenshot) -> Result<Self, KickbotError> {
        self.recognise_from_mat(&image.image)
    }

    pub fn get_text(&mut self) -> Result<String, KickbotError> {
        Ok(self
            .tesseract
            .get_text()
            .map_err(|err| KickbotError::TesseractError(err.to_string()))?)
    }
}
