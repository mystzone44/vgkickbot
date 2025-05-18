use crate::errors::KickbotError;
use crate::recognition::screenshot::Screenshot;
use ndarray::{ArrayBase, Axis, CowRepr, Ix2, Ix3, Ix4, OwnedRepr};
use opencv::core::{Mat, MatTraitConst, MatTraitConstManual, Vector};
use opencv::{self as cv};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum WeaponClasses {
    AllowedPrimaryGuns,
    HeavyBomber,
    HMG,
    LMG,
    SMG08,
}

pub struct Classifier {
    model: Session,
}

impl Classifier {
    pub fn new() -> Self {
        let model = Session::builder()
            .unwrap()
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .unwrap()
            .with_intra_threads(4)
            .unwrap()
            .commit_from_file("bf1ai.onnx")
            .unwrap();
        Classifier { model }
    }

    fn center_crop(&self, image: &Mat, size: cv::core::Size) -> Mat {
        let (width, height) = (image.cols(), image.rows());
        let left = (width - size.width) / 2;
        let top = (height - size.height) / 2;
        let right = (width + size.width) / 2;
        let bottom = (height + size.height) / 2;

        let width = right - left;
        let height = bottom - top;
        image
            .roi(cv::core::Rect::new(left, top, width, height))
            .unwrap()
            .clone_pointee()
    }

    fn preprocess(
        &self,
        image: &Screenshot,
    ) -> Result<ArrayBase<OwnedRepr<f32>, Ix4>, KickbotError> {
        let mut modified_image = Mat::default();
        cv::imgproc::resize(
            &image.image,
            &mut modified_image,
            cv::core::Size::new(256, 256),
            0f64,
            0f64,
            cv::imgproc::INTER_LINEAR,
        )?;

        modified_image = self.center_crop(&modified_image, cv::core::Size::new(224, 224));

        let mean: [f32; 3] = [0.485, 0.456, 0.406];
        let std: [f32; 3] = [0.229, 0.224, 0.225];

        let mut img_array = ndarray::Array::from_shape_vec(
            (3, 224, 224),
            modified_image
                .data_bytes()?
                .chunks(4)
                .flat_map(|px| {
                    px.iter()
                        .take(3)
                        .map(|&v| v as f32 / 255.0)
                        .collect::<Vec<_>>()
                })
                .collect(),
        )
        .unwrap();

        for x in 0..img_array.shape()[1] {
            for y in 0..img_array.shape()[2] {
                img_array[[0, x, y]] = (img_array[[0, x, y]] - mean[0]) / std[0];
                img_array[[1, x, y]] = (img_array[[1, x, y]] - mean[1]) / std[1];
                img_array[[2, x, y]] = (img_array[[2, x, y]] - mean[2]) / std[2];
            }
        }

        img_array = img_array.permuted_axes([2, 0, 1]);
        let mut new_axis_img_array = img_array.insert_axis(Axis(0));
        new_axis_img_array.swap_axes(1, 2);

        Ok(new_axis_img_array)
    }

    pub fn infer(&self, image: &Screenshot) -> Result<(f32, WeaponClasses), KickbotError> {
        let image_array = self.preprocess(image)?;

        let tensor = Tensor::from_array(image_array)?;
        let outputs = self.model.run(ort::inputs![tensor]?)?;
        let predictions = outputs[0].try_extract_tensor::<f32>()?;

        let predictions_max = predictions.iter().cloned().reduce(f32::max).unwrap();
        let exp_scores = predictions
            .iter()
            .map(|&v| v - predictions_max)
            .collect::<Vec<f32>>();
        let exp_sum: f32 = exp_scores.iter().sum();

        let probabilities = exp_scores.iter().map(|v| v / exp_sum).collect::<Vec<f32>>();
        let (top_probability, top_prediction) = probabilities
            .iter()
            .zip(0..probabilities.len())
            .reduce(|(acc_v, acc_idx), (v, idx)| {
                if v > acc_v {
                    (v, idx)
                } else {
                    (acc_v, acc_idx)
                }
            })
            .unwrap();

        let prediction_class = match top_prediction {
            0 => WeaponClasses::AllowedPrimaryGuns,
            1 => WeaponClasses::HeavyBomber,
            2 => WeaponClasses::HMG,
            3 => WeaponClasses::LMG,
            4 => WeaponClasses::SMG08,
            _ => panic!("Prediction class out of bounds"),
        };

        Ok((*top_probability, prediction_class))
    }
}
