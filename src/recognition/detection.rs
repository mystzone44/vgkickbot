use crate::config::{Config, Vehicle, Weapon};
use crate::errors::KickbotError;
use crate::recognition::enhance::enhance_image;
use crate::recognition::model::{Classifier, WeaponClasses};
use crate::recognition::ocr::OCR;
use crate::recognition::screenshot::Screenshot;
use opencv::core::Rect;
use std::slice::Iter;
use std::vec::IntoIter;

/*fn detect_gadgets(
    config: &Config,
    mut ocr: OCR,
    screenshot: &Screenshot,
) -> Result<OCR, KickbotError> {
    let gadget_slot_1 = screenshot.crop_image(config.gadget_slot1_box)?;
    let gadget_slot_2 = screenshot.crop_image(config.gadget_slot2_box)?;
    ocr = ocr.recognise_from_screenshot(&gadget_slot_1)?;
    let gadget_slot_1_text = ocr.get_text()?;
    ocr = ocr.recognise_from_screenshot(&gadget_slot_2)?;
    let gadget_slot_2_text = ocr.get_text()?;

    Ok(ocr)
} */

fn read_weapon_slot(
    slot_rect: Rect,
    mut ocr: OCR,
    screenshot: &Screenshot,
) -> Result<(OCR, String), KickbotError> {
    let weapon_slot = screenshot.crop_image(slot_rect)?;
    ocr = ocr.recognise_from_screenshot(&weapon_slot)?;
    let weapon_slot_text = ocr.get_text()?;

    Ok((ocr, weapon_slot_text))
}

fn find_similar(mut names: Iter<String>, weapon_name: &str, config: &Config) -> bool {
    names
        .find(|name| config.are_similar(name, weapon_name, config.weapon_similar_name_probability))
        .is_some()
}

struct Detector {
    pub slot1: String,
    pub slot2: Option<String>,
}

impl Detector {
    fn smg_slot1(
        &self,
        ocr: OCR,
        config: &Config,
        screenshot: &Screenshot,
    ) -> Result<(OCR, Option<String>), KickbotError> {
        let (ocr, slot_text) = read_weapon_slot(config.weapon_name_slot1_box, ocr, &screenshot)?;

        Ok((
            ocr,
            match find_similar(
                config.banned_weapon.names.iter(),
                slot_text.as_str(),
                config,
            ) {
                true => Some(config.banned_weapon.pretty_name.clone()),
                false => None,
            },
        ))
    }

    fn heavy_bomber_slot1_slot2(
        &mut self,
        mut ocr: OCR,
        config: &Config,
        screenshot: &Screenshot,
        use_or: bool,
    ) -> Result<(OCR, Option<String>), KickbotError> {
        let heavy_bomber = &config.banned_vehicles[&WeaponClasses::HeavyBomber];

        let slot1_result = find_similar(
            heavy_bomber.primary_names.iter(),
            self.slot1.as_str(),
            config,
        );

        if use_or && slot1_result {
            return Ok((ocr, Some(heavy_bomber.pretty_name.clone())));
        }

        if self.slot2.is_none() {
            let mut slot2_text = String::new();
            (ocr, slot2_text) = read_weapon_slot(config.weapon_name_slot2_box, ocr, &screenshot)?;
            self.slot2 = Some(slot2_text)
        }

        let slot2_result = find_similar(
            heavy_bomber.secondary_names.iter(),
            self.slot2.clone().unwrap().as_str(),
            config,
        );

        if use_or {
            return Ok((ocr, Some(heavy_bomber.pretty_name.clone())));
        } else {
            if slot1_result && slot2_result {
                return Ok((ocr, Some(heavy_bomber.pretty_name.clone())));
            }
        }

        Ok((ocr, None))
    }

    fn lmg_slot2(
        &mut self,
        mut ocr: OCR,
        config: &Config,
        screenshot: &Screenshot,
    ) -> Result<(OCR, Option<String>), KickbotError> {
        if self.slot2.is_none() {
            let mut slot2_text = String::new();
            (ocr, slot2_text) = read_weapon_slot(config.weapon_name_slot2_box, ocr, &screenshot)?;
            self.slot2 = Some(slot2_text)
        }

        let mortar_truck = &config.banned_vehicles[&WeaponClasses::LMG];

        if find_similar(
            mortar_truck.secondary_names.iter(),
            self.slot2.clone().unwrap().as_str(),
            config,
        ) {
            Ok((ocr, Some(mortar_truck.pretty_name.clone())))
        } else {
            Ok((ocr, None))
        }
    }
}

pub fn detect_player_name(
    screenshot: &Screenshot,
    config: &Config,
    mut ocr: OCR,
) -> Result<(OCR, Option<String>), KickbotError> {
    let player_name_image = enhance_image(
        &screenshot.crop_image(config.player_name_box)?,
        config.ally_colour,
        config.enemy_colour,
    )?
    .0;

    ocr = ocr.recognise_from_mat(&player_name_image)?;
    let player_name = ocr.get_text().ok();
    Ok((ocr, player_name))
}

pub fn detect(
    screenshot: &Screenshot,
    config: &Config,
    ocr: OCR,
    classifier: &Classifier,
) -> Result<(OCR, Option<String>, Option<WeaponClasses>), KickbotError> {
    let weapon_icon_image = screenshot.crop_image(config.weapon_icon_box)?;
    let (probability, category) = classifier.infer(&weapon_icon_image)?;

    let (mut ocr, slot1) = read_weapon_slot(config.weapon_name_slot1_box, ocr, &screenshot)?;
    let mut detector = Detector { slot1, slot2: None };

    if probability < config.weapon_icon_probability {
        let mut maybe_banned_weapon = None;
        // No icon detected, just try text
        (ocr, maybe_banned_weapon) = detector.smg_slot1(ocr, config, &screenshot)?;
        if maybe_banned_weapon.is_some() {
            return Ok((ocr, maybe_banned_weapon, Some(WeaponClasses::SMG08)));
        }

        (ocr, maybe_banned_weapon) =
            detector.heavy_bomber_slot1_slot2(ocr, config, &screenshot, false)?;
        if maybe_banned_weapon.is_some() {
            return Ok((ocr, maybe_banned_weapon, Some(WeaponClasses::HeavyBomber)));
        }

        if config.are_similar(
            detector.slot1.as_str(),
            "LMG",
            config.weapon_similar_name_probability,
        ) {
            (ocr, maybe_banned_weapon) = detector.lmg_slot2(ocr, config, &screenshot)?;
            if maybe_banned_weapon.is_some() {
                return Ok((ocr, maybe_banned_weapon, Some(WeaponClasses::LMG)));
            }
        }

        return Ok((ocr, None, None));
    }

    let (ocr, maybe_banned_weapon) = match category {
        WeaponClasses::AllowedPrimaryGuns => (ocr, None),
        // Confirm via slot 1 text OR slot 2 text
        WeaponClasses::HeavyBomber => {
            detector.heavy_bomber_slot1_slot2(ocr, config, screenshot, true)?
        }
        // Confirm via slot 2 text
        WeaponClasses::LMG => detector.lmg_slot2(ocr, config, screenshot)?,
        // Confirm via slot 1 text
        WeaponClasses::SMG08 => detector.smg_slot1(ocr, config, &screenshot)?,
        _ => (ocr, None),
    };

    if maybe_banned_weapon.is_some() {
        return Ok((ocr, maybe_banned_weapon, Some(category)));
    }

    Ok((ocr, None, None))
}
