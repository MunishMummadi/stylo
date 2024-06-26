/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

<%namespace name="helpers" file="/helpers.mako.rs" />

<%helpers:shorthand name="mask" engines="gecko" extra_prefixes="webkit"
                    flags="SHORTHAND_IN_GETCS"
                    sub_properties="mask-mode mask-repeat mask-clip mask-origin mask-composite mask-position-x
                                    mask-position-y mask-size mask-image"
                    spec="https://drafts.fxtf.org/css-masking/#propdef-mask">
    use crate::properties::longhands::{mask_mode, mask_repeat, mask_clip, mask_origin, mask_composite, mask_position_x,
                                mask_position_y};
    use crate::properties::longhands::{mask_size, mask_image};
    use crate::values::specified::{Position, PositionComponent};
    use crate::parser::Parse;

    // FIXME(emilio): These two mask types should be the same!
    impl From<mask_origin::single_value::SpecifiedValue> for mask_clip::single_value::SpecifiedValue {
        fn from(origin: mask_origin::single_value::SpecifiedValue) -> mask_clip::single_value::SpecifiedValue {
            match origin {
                mask_origin::single_value::SpecifiedValue::ContentBox =>
                    mask_clip::single_value::SpecifiedValue::ContentBox,
                mask_origin::single_value::SpecifiedValue::PaddingBox =>
                    mask_clip::single_value::SpecifiedValue::PaddingBox ,
                mask_origin::single_value::SpecifiedValue::BorderBox =>
                    mask_clip::single_value::SpecifiedValue::BorderBox,
                % if engine == "gecko":
                mask_origin::single_value::SpecifiedValue::FillBox =>
                    mask_clip::single_value::SpecifiedValue::FillBox ,
                mask_origin::single_value::SpecifiedValue::StrokeBox =>
                    mask_clip::single_value::SpecifiedValue::StrokeBox,
                mask_origin::single_value::SpecifiedValue::ViewBox=>
                    mask_clip::single_value::SpecifiedValue::ViewBox,
                % endif
            }
        }
    }

    pub fn parse_value<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Longhands, ParseError<'i>> {
        % for name in "image mode position_x position_y size repeat origin clip composite".split():
        // Vec grows from 0 to 4 by default on first push().  So allocate with
        // capacity 1, so in the common case of only one item we don't way
        // overallocate, then shrink.  Note that we always push at least one
        // item if parsing succeeds.
        let mut mask_${name} = Vec::with_capacity(1);
        % endfor

        input.parse_comma_separated(|input| {
            % for name in "image mode position size repeat origin clip composite".split():
                let mut ${name} = None;
            % endfor
            loop {
                if image.is_none() {
                    if let Ok(value) = input.try_parse(|input| mask_image::single_value
                                                                   ::parse(context, input)) {
                        image = Some(value);
                        continue
                    }
                }
                if position.is_none() {
                    if let Ok(value) = input.try_parse(|input| Position::parse(context, input)) {
                        position = Some(value);

                        // Parse mask size, if applicable.
                        size = input.try_parse(|input| {
                            input.expect_delim('/')?;
                            mask_size::single_value::parse(context, input)
                        }).ok();

                        continue
                    }
                }
                % for name in "repeat origin clip composite mode".split():
                    if ${name}.is_none() {
                        if let Ok(value) = input.try_parse(|input| mask_${name}::single_value
                                                                               ::parse(context, input)) {
                            ${name} = Some(value);
                            continue
                        }
                    }
                % endfor
                break
            }
            if clip.is_none() {
                if let Some(origin) = origin {
                    clip = Some(mask_clip::single_value::SpecifiedValue::from(origin));
                }
            }
            let mut any = false;
            % for name in "image mode position size repeat origin clip composite".split():
                any = any || ${name}.is_some();
            % endfor
            if any {
                if let Some(position) = position {
                    mask_position_x.push(position.horizontal);
                    mask_position_y.push(position.vertical);
                } else {
                    mask_position_x.push(PositionComponent::zero());
                    mask_position_y.push(PositionComponent::zero());
                }
                % for name in "image mode size repeat origin clip composite".split():
                    if let Some(m_${name}) = ${name} {
                        mask_${name}.push(m_${name});
                    } else {
                        mask_${name}.push(mask_${name}::single_value
                                                        ::get_initial_specified_value());
                    }
                % endfor
                Ok(())
            } else {
                Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
            }
        })?;

        Ok(expanded! {
            % for name in "image mode position_x position_y size repeat origin clip composite".split():
                mask_${name}: mask_${name}::SpecifiedValue(mask_${name}.into()),
            % endfor
         })
    }

    impl<'a> ToCss for LonghandsToSerialize<'a>  {
        fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result where W: fmt::Write {
            use crate::properties::longhands::mask_origin::single_value::computed_value::T as Origin;
            use crate::properties::longhands::mask_clip::single_value::computed_value::T as Clip;
            use style_traits::values::SequenceWriter;

            let len = self.mask_image.0.len();
            if len == 0 {
                return Ok(());
            }
            % for name in "mode position_x position_y size repeat origin clip composite".split():
                if self.mask_${name}.0.len() != len {
                    return Ok(());
                }
            % endfor

            // For each <mask-layer>, we serialize it according to the following order:
            // <mask-layer> =
            //   <mask-reference> ||
            //   <position> [ / <bg-size> ]? ||
            //   <repeat-style> ||
            //   <coord-box> ||
            //   [ <coord-box> | no-clip ] ||
            //   <compositing-operator> ||
            //   <masking-mode>
            // https://drafts.fxtf.org/css-masking-1/#the-mask
            for i in 0..len {
                if i > 0 {
                    dest.write_str(", ")?;
                }

                % for name in "image mode position_x position_y size repeat origin clip composite".split():
                    let ${name} = &self.mask_${name}.0[i];
                % endfor

                let mut has_other = false;
                % for name in "image mode size repeat composite".split():
                    let has_${name} =
                        *${name} != mask_${name}::single_value::get_initial_specified_value();
                    has_other |= has_${name};
                % endfor
                let has_position = *position_x != PositionComponent::zero()
                    || *position_y != PositionComponent::zero();
                let has_origin = *origin != Origin::BorderBox;
                let has_clip = *clip != Clip::BorderBox;

                // If all are initial values, we serialize mask-image.
                if !has_other && !has_position && !has_origin && !has_clip {
                    return image.to_css(dest);
                }

                let mut writer = SequenceWriter::new(dest, " ");
                // <mask-reference>
                if has_image {
                    writer.item(image)?;
                }

                // <position> [ / <bg-size> ]?
                if has_position || has_size {
                    writer.item(&Position {
                        horizontal: position_x.clone(),
                        vertical: position_y.clone()
                    })?;

                    if has_size {
                        writer.raw_item("/")?;
                        writer.item(size)?;
                    }
                }

                // <repeat-style>
                if has_repeat {
                    writer.item(repeat)?;
                }

                // <coord-box>
                // Note:
                // Even if 'mask-origin' is at its initial value 'border-box',
                // we still have to serialize it to avoid ambiguity iF the
                // 'mask-clip' longhand has some other <coord-box> value
                // (i.e. neither 'border-box' nor 'no-clip'). (If we naively
                // declined to serialize the 'mask-origin' value in this
                // situation, then whatever value we serialize for 'mask-clip'
                // would implicitly also represent 'mask-origin' and would be
                // providing the wrong value for that longhand.)
                if has_origin || (has_clip && *clip != Clip::NoClip) {
                    writer.item(origin)?;
                }

                // [ <coord-box> | no-clip ]
                if has_clip && *clip != From::from(*origin) {
                    writer.item(clip)?;
                }

                // <compositing-operator>
                if has_composite {
                    writer.item(composite)?;
                }

                // <masking-mode>
                if has_mode {
                    writer.item(mode)?;
                }
            }

            Ok(())
        }
    }
</%helpers:shorthand>

<%helpers:shorthand name="mask-position" engines="gecko" extra_prefixes="webkit"
                    flags="SHORTHAND_IN_GETCS"
                    sub_properties="mask-position-x mask-position-y"
                    spec="https://drafts.csswg.org/css-masks-4/#the-mask-position">
    use crate::properties::longhands::{mask_position_x,mask_position_y};
    use crate::values::specified::position::Position;
    use crate::parser::Parse;

    pub fn parse_value<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Longhands, ParseError<'i>> {
        // Vec grows from 0 to 4 by default on first push().  So allocate with
        // capacity 1, so in the common case of only one item we don't way
        // overallocate, then shrink.  Note that we always push at least one
        // item if parsing succeeds.
        let mut position_x = Vec::with_capacity(1);
        let mut position_y = Vec::with_capacity(1);
        let mut any = false;

        input.parse_comma_separated(|input| {
            let value = Position::parse(context, input)?;
            position_x.push(value.horizontal);
            position_y.push(value.vertical);
            any = true;
            Ok(())
        })?;

        if !any {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }


        Ok(expanded! {
            mask_position_x: mask_position_x::SpecifiedValue(position_x.into()),
            mask_position_y: mask_position_y::SpecifiedValue(position_y.into()),
        })
    }

    impl<'a> ToCss for LonghandsToSerialize<'a>  {
        fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result where W: fmt::Write {
            let len = self.mask_position_x.0.len();
            if len == 0 || self.mask_position_y.0.len() != len {
                return Ok(());
            }

            for i in 0..len {
                Position {
                    horizontal: self.mask_position_x.0[i].clone(),
                    vertical: self.mask_position_y.0[i].clone()
                }.to_css(dest)?;

                if i < len - 1 {
                    dest.write_str(", ")?;
                }
            }

            Ok(())
        }
    }
</%helpers:shorthand>
