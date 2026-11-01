//! D2D saturation and Gaussian blur described as Windows.Graphics.Effects for the system compositor.
use windows::{
    Win32::Foundation::E_INVALIDARG,
    core::*,
    Foundation::{IPropertyValue, PropertyValue},
    Graphics::Effects::{IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffect_Impl, IGraphicsEffectSource_Impl},
    Win32::Graphics::Direct2D::{
        Common::D2D1_BORDER_MODE_HARD, CLSID_D2D1GaussianBlur, CLSID_D2D1Saturation, D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED,
    },
    Win32::System::WinRT::Graphics::Direct2D::{
        IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl, GRAPHICS_EFFECT_PROPERTY_MAPPING,
        GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT,
    },
};

#[derive(Clone, Copy)]
enum Kind {
    Saturation,
    Blur,
}

#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct Effect {
    kind: Kind,
    amount: f32,
    source: IGraphicsEffectSource,
    name: std::cell::RefCell<HSTRING>,
}

pub(super) fn saturation(source: &IGraphicsEffectSource, amount: f32) -> IGraphicsEffect {
    Effect { kind: Kind::Saturation, amount, source: source.clone(), name: HSTRING::from("Saturation").into() }.into()
}

pub(super) fn blur(source: &IGraphicsEffectSource, amount: f32) -> IGraphicsEffect {
    Effect { kind: Kind::Blur, amount, source: source.clone(), name: HSTRING::from("Blur").into() }.into()
}

impl IGraphicsEffect_Impl for Effect_Impl {
    fn Name(&self) -> Result<HSTRING> {
        Ok(self.name.borrow().clone())
    }
    fn SetName(&self, name: &HSTRING) -> Result<()> {
        *self.name.borrow_mut() = name.clone();
        Ok(())
    }
}

impl IGraphicsEffectSource_Impl for Effect_Impl {}

impl IGraphicsEffectD2D1Interop_Impl for Effect_Impl {
    fn GetEffectId(&self) -> Result<GUID> {
        Ok(match self.kind {
            Kind::Saturation => CLSID_D2D1Saturation,
            Kind::Blur => CLSID_D2D1GaussianBlur,
        })
    }
    fn GetNamedPropertyMapping(
        &self,
        name: &PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> Result<()> {
        let name = unsafe { name.to_string() }.map_err(|_| Error::from(E_INVALIDARG))?;
        let found = match (self.kind, name.as_str()) {
            (Kind::Saturation, "Saturation") | (Kind::Blur, "BlurAmount") => 0,
            (Kind::Blur, "Optimization") => 1,
            (Kind::Blur, "BorderMode") => 2,
            _ => return Err(E_INVALIDARG.into()),
        };
        unsafe {
            *index = found;
            *mapping = GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT;
        }
        Ok(())
    }
    fn GetPropertyCount(&self) -> Result<u32> {
        Ok(match self.kind {
            Kind::Saturation => 1,
            Kind::Blur => 3,
        })
    }
    fn GetProperty(&self, index: u32) -> Result<IPropertyValue> {
        // A Gaussian blur needs all three D2D properties described, not just the amount.
        let value = match (self.kind, index) {
            (_, 0) => PropertyValue::CreateSingle(self.amount)?,
            (Kind::Blur, 1) => PropertyValue::CreateUInt32(D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED.0 as u32)?,
            (Kind::Blur, 2) => PropertyValue::CreateUInt32(D2D1_BORDER_MODE_HARD.0 as u32)?,
            _ => return Err(E_INVALIDARG.into()),
        };
        value.cast()
    }
    fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
        if index != 0 {
            return Err(E_INVALIDARG.into());
        }
        Ok(self.source.clone())
    }
    fn GetSourceCount(&self) -> Result<u32> {
        Ok(1)
    }
}
