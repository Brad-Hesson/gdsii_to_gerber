use std::{
    fs::File,
    io::{BufWriter, Write},
};

use anyhow::Result;
use gds21::{GdsElement, GdsLibrary, GdsPoint, GdsStructRef};
use gerber_types::{CoordinateNumber, GerberResult};
use thiserror::Error;

fn main() -> Result<()> {
    let lib =
        gds21::GdsLibrary::load(r#"C:\Users\slice\code\actuator-project\klayout\stencil.GDS"#)
            .unwrap();
    let name = "300um";
    let pat = Pattern::from_gds_struct(&lib, name)?;
    let mut w = BufWriter::new(File::create(format!("{name}.pcb"))?);
    pat.write_gerber(&mut w, &lib)?;
    Ok(())
}

#[derive(Debug)]
struct Pattern(Vec<Region>);

impl Pattern {
    fn from_gds_struct(lib: &GdsLibrary, name: &str) -> PatternResult<Self> {
        let struc = lib
            .structs
            .iter()
            .find(|s| s.name == name)
            .ok_or(PatternError::PatternDoesNotExist)?;
        let mut regions: Vec<Region> = vec![];
        for elem in &struc.elems {
            match elem {
                GdsElement::GdsBoundary(b) => regions.push(b.xy.iter().collect()),
                GdsElement::GdsStructRef(GdsStructRef { name, xy, .. }) => {
                    let pat = Pattern::from_gds_struct(lib, name)? + xy.into();
                    regions.extend(pat.0);
                }
                _ => unimplemented!(),
            }
        }
        Ok(Self(regions))
    }
    fn write_gerber(&self, w: &mut impl Write, lib: &GdsLibrary) -> GerberResult<()> {
        use gerber_types::*;
        let co_fmt = CoordinateFormat::new(6, 6);
        ExtendedCode::CoordinateFormat(co_fmt).serialize(w)?;
        ExtendedCode::Unit(gerber_types::Unit::Millimeters).serialize(w)?;
        GCode::RegionMode(true).serialize(w)?;
        for region in &self.0 {
            DCode::Operation(Operation::Move(Coordinates {
                x: Some(coord_from_gds(region.0[0].x, lib)),
                y: Some(coord_from_gds(region.0[0].y, lib)),
                format: co_fmt,
            }))
            .serialize(w)?;
            for point in &region.0 {
                DCode::Operation(gerber_types::Operation::Interpolate(
                    Coordinates {
                        x: Some(coord_from_gds(point.x, lib)),
                        y: Some(coord_from_gds(point.y, lib)),
                        format: co_fmt,
                    },
                    None,
                ))
                .serialize(w)?;
            }
        }
        GCode::RegionMode(false).serialize(w)?;
        MCode::EndOfFile.serialize(w)?;
        Ok(())
    }
}

fn coord_from_gds(v: i32, lib: &GdsLibrary) -> CoordinateNumber {
    let unit = lib.units.db_unit();
    let meters = v as f64 * unit;
    let millis = meters * 1000.;
    <CoordinateNumber as conv::TryFrom<f64>>::try_from(millis).unwrap()
}

impl std::ops::Add<Point> for Pattern {
    type Output = Pattern;

    fn add(mut self, rhs: Point) -> Self::Output {
        for r in &mut self.0 {
            *r += rhs;
        }
        self
    }
}

#[derive(Debug)]
struct Region(Vec<Point>);
impl<I> FromIterator<I> for Region
where
    I: Into<Point>,
{
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        Self(iter.into_iter().map(|v| v.into()).collect())
    }
}
impl std::ops::AddAssign<Point> for Region {
    fn add_assign(&mut self, rhs: Point) {
        for p in &mut self.0 {
            *p = *p + rhs;
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}
impl std::ops::Add for Point {
    type Output = Point;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}
impl From<&GdsPoint> for Point {
    fn from(p: &GdsPoint) -> Self {
        Self { x: p.x, y: p.y }
    }
}

type PatternResult<T> = Result<T, PatternError>;

#[derive(Error, Debug)]
enum PatternError {
    #[error("The requested pattern name does not exist in the library")]
    PatternDoesNotExist,
}
