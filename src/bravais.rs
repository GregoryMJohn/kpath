use std::{
	cmp::Ordering::{Equal, Greater, Less},
	collections::HashMap,
};

pub enum BravaisLattice {
	Triclinic(Tri),
	Monoclinic(Mcl),
	Orthorhombic(Orc),
	Tetragonal(Tet),
	Rhombohedral(Rhl),
	Hexagonal,
	Cubic(Cub),
}

impl BravaisLattice {
	pub fn from_space_group_number(
		space_group_num: u8,
		params: &LatticeParameters,
	) -> Result<Self, String> {
		use BravaisLattice::*;
		use Cub::*;
		use Mcl::*;
		use Orc::*;
		use Rhl::*;
		use Tet::*;
		use Tri::*;

		let a = params.a();
		let b = params.b();
		let c = params.c();
		let a2 = a * a;
		let b2 = b * b;
		let c2 = c * c;
		let α = params.α();
		let β = params.β();
		let γ = params.γ();

		let result = match space_group_num {
			| 0 | 231..=u8::MAX => {
				return Err(String::from("Invalid space group"))
			}
			| 1 => Triclinic(P1),
			| 2 => Triclinic(P1Minus),
			| 3 | 4 | 6 | 7 | 10 | 11 | 13 | 14 => {
				if a < b && b <= c && α < 90.0 && β == 90.0 && γ == 90.0 {
					Monoclinic(MCL)
				} else {
					return Err(String::from(
						"Unexpected ordering of lattice parameters for monoclinic space group. Expected ordering: a < b <= c, α < 90.0°, β = γ = 90.0°.",
					))
				}
			}
			| 5 | 8 | 9 | 12 | 15 => {
				if a < b && b <= c && α < 90.0 && β == 90.0 && γ == 90.0 {
					let a_1 = &[a / 2., b / 2., 0.];
					let a_2 = &[-a / 2., b / 2., 0.];

					let b_1 =
						a_1.iter().map(|elem| 1. / *elem).collect::<Vec<_>>();
					let b_1 = b_1.as_array::<3>().unwrap();
					let b_2 =
						a_2.iter().map(|elem| 1. / *elem).collect::<Vec<_>>();
					let b_2 = b_2.as_array::<3>().unwrap();

					let b1mag =
						b_1.iter().zip(b_1).map(|(a, b)| a * b).sum::<f64>();
					let b2mag =
						b_2.iter().zip(b_2).map(|(a, b)| a * b).sum::<f64>();

					let b1dotb2 = b_1
						.iter()
						.zip(b_2.iter())
						.map(|(a, b)| a * b)
						.sum::<f64>();

					let kγ = f64::acos(b1dotb2 / (b1mag * b2mag));

					let mcl = match kγ.partial_cmp(&90.0).unwrap() {
						| Greater => MCLC1,
						| Equal => MCLC2,
						| Less => match (b * α.cos() / c
							+ b2 * α.sin().powf(2.) / a2)
							.partial_cmp(&1.)
							.unwrap()
						{
							| Less => MCLC3,
							| Equal => MCLC4,
							| Greater => MCLC5,
						},
					};

					Monoclinic(mcl)
				} else {
					return Err(String::from(
						"Unexpected ordering of lattice parameters for monoclinic space group. Expected ordering: a < b <= c, α < 90.0°, β = γ = 90.0°.",
					))
				}
			}
			| 16..=19 | 25..=34 | 47..=62 => Orthorhombic(ORC),
			| 20 | 21 | 35..=41 | 63..=68 => Orthorhombic(ORCC),
			| 22 | 42 | 43 | 69 | 70 => {
				match (1. / a2).partial_cmp(&(1. / b2 + 1. / c2)).unwrap() {
					| Greater => Orthorhombic(ORCF1),
					| Less => Orthorhombic(ORCF2),
					| Equal => Orthorhombic(ORCF3),
				}
			}
			| 23 | 24 | 44..=46 | 71..=74 => Orthorhombic(ORCI),
			| 75..=78
			| 81
			| 83..=86
			| 89..=96
			| 99..=106
			| 111..=118
			| 123..=138 => Tetragonal(TET),
			| 79
			| 80
			| 82
			| 87
			| 88
			| 97
			| 98
			| 107..=110
			| 119..=122
			| 139..=142 => match a.partial_cmp(&c).unwrap() {
				| Less => Tetragonal(BCT1),
				| Greater => Tetragonal(BCT2),
				| Equal => {
					return Err(String::from(
						"a = c is not compatible with the provided (body-centered tetragonal) space group",
					))
				}
			},
			| 195 | 198 | 200 | 201 | 205 | 207 | 208 | 212 | 213 | 215
			| 218 | 221 | 223 | 226 | 229 => Cubic(CUB),
			| 146 | 148 | 155 | 160 | 161 | 166 | 167 => {
				match α.partial_cmp(&90.0).unwrap() {
					| Less => Rhombohedral(RHL1),
					| Greater => Rhombohedral(RHL2),
					| Equal => {
						return Err(String::from(
							"α = 90.0° is not compatible with the provided (rhombohedral) space group",
						))
					}
				}
			}
			| 143..=145
			| 147
			| 149..=154
			| 156..=159
			| 162..=165
			| 168..=194 => Hexagonal,
			| 196 | 202 | 203 | 209 | 210 | 216 | 219 | 222 | 224 | 227
			| 230 => Cubic(FCC),
			| 197 | 199 | 204 | 206 | 211 | 214 | 217 | 220 | 225 | 228 => {
				Cubic(BCC)
			}
		};

		Ok(result)
	}

	pub fn kpoints(&self, params: &LatticeParameters) -> KPoints {
		use BravaisLattice::*;
		match self {
			| Triclinic(tri) => {
				use Tri::*;

				match tri {
					| P1 => KPoints::from([
						("Γ", [0.0, 0.0, 0.0]),
						("L", [0.5, 0.5, 0.0]),
						("M", [0.0, 0.5, 0.5]),
						("N", [0.5, 0.0, 0.5]),
						("R", [0.5, 0.5, 0.5]),
						("X", [0.5, 0.0, 0.0]),
						("Y", [0.0, 0.5, 0.0]),
						("Z", [0.0, 0.0, 0.5]),
					]),
					| P1Minus => KPoints::from([
						("Γ", [0.0, 0.0, 0.0]),
						("L", [0.5, -0.5, 0.0]),
						("M", [0.0, 0.0, 0.5]),
						("N", [-0.5, -0.5, 0.5]),
						("R", [0.0, -0.5, 0.5]),
						("X", [0.0, -0.5, 0.0]),
						("Y", [0.5, 0.0, 0.0]),
						("Z", [-0.5, 0.0, 0.5]),
					]),
				}
			}
			| Monoclinic(mcl) => {
				use Mcl::*;
				let a = params.a();
				let b = params.b();
				let c = params.c();
				let a2 = a * a;
				let b2 = b * b;

				let α = params.α();

				match mcl {
					| MCL => {
						let n =
							(1. - b * α.cos() / c) / (2. * α.sin().powf(2.));

						let v = 0.5 - n * c * α.cos() / b;

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("A", [0.5, 0.5, 0.]),
							("C", [0., 0.5, 0.5]),
							("D", [0.5, 0., 0.5]),
							("D₁", [0.5, 0., -0.5]),
							("E", [0.5, 0.5, 0.5]),
							("H", [0., n, 1. - v]),
							("H₁", [0., 1. - n, v]),
							("H₂", [0., n, -v]),
							("M", [0.5, n, 1. - v]),
							("M₁", [0.5, 1. - n, v]),
							("M₂", [0.5, n, -v]),
							("X", [0., 0.5, 0.]),
							("Y", [0., 0., 0.5]),
							("Y₁", [0., 0., -0.5]),
							("Z", [0.5, 0., 0.]),
						])
					}
					| MCLC1 | MCLC2 => {
						let ζ =
							(2. - b * α.cos() / c) / (4. * α.sin().powf(2.));
						let n = 0.5 + 2. * ζ * c * α.cos() / b;
						let Ψ = 0.75 - a2 / (4. * b2 * α.sin().powf(2.));
						let φ = Ψ + (0.75 - Ψ) * b * α.cos() / c;

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("N", [0.5, 0., 0.]),
							("N₁", [0., -0.5, 0.]),
							("F", [1. - ζ, 1. - ζ, 1. - n]),
							("F₁", [ζ, ζ, n]),
							("F₂", [-ζ, -ζ, 1. - n]),
							("F₃", [1. - ζ, -ζ, 1. - n]),
							("I", [φ, 1. - φ, 0.5]),
							("I₁", [1. - φ, φ - 1., 0.5]),
							("L", [0.5, 0.5, 0.5]),
							("M", [0.5, 0., 0.5]),
							("X", [1. - Ψ, Ψ - 1., 0.]),
							("X₁", [Ψ, 1. - Ψ, 0.]),
							("X₂", [Ψ - 1., -Ψ, 0.]),
							("Y", [0.5, 0.5, 0.]),
							("Y₁", [-0.5, -0.5, 0.]),
							("Z", [0., 0., 0.5]),
						])
					}
					| MCLC3 | MCLC4 => {
						let μ = 0.25 * (1. + b2 / a2);
						let δ = b * c * α.cos() / (2. * a2);
						let ζ = μ - 0.25
							+ (1. - b * α.cos() / c) / (4. * α.sin().powf(2.));
						let n = 0.5 + 2. * ζ * c * α.cos() / b;
						let φ = 1. + ζ - 2. * μ;
						let Ψ = n - 2. * δ;

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("F", [1. - φ, 1. - φ, 1. - Ψ]),
							("F₁", [φ, φ - 1., Ψ]),
							("F₂", [1. - φ, -φ, 1. - Ψ]),
							("H", [ζ, ζ, n]),
							("H₁", [1. - ζ, 1. - ζ, 1. - n]),
							("H₂", [-ζ, -ζ, 1. - n]),
							("I", [0.5, -0.5, 0.5]),
							("M", [0.5, 0., 0.5]),
							("N", [0.5, 0., 0.]),
							("N₁", [0., -0.5, 0.]),
							("X", [0.5, -0.5, 0.]),
							("Y", [μ, μ, δ]),
							("Y₁", [1. - μ, -μ, -δ]),
							("Y₂", [-μ, -μ, -δ]),
							("Y₃", [μ, μ - 1., δ]),
							("Z", [0., 0., 0.5]),
						])
					}
					| MCLC5 => {
						let ζ = 0.25
							* (b2 / a2
								+ (1. - b * α.cos() / c) / α.sin().powf(2.));

						let n = 0.5 + 2. * ζ * c * α.cos() / b;
						let μ = 0.5 * n + b2 / (4. * a2)
							- b * c * α.cos() / (2. * a2);
						let v = 2. * μ - ζ;
						let ρ = 1. - ζ * a2 / b2;
						let ω = (4. * v - 1. - b2 * α.sin().powf(2.) / a2) * c
							/ (2. * b * α.cos());
						let δ = ζ * c * α.cos() / b + ω * 0.5 - 0.25;

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("F", [v, v, ω]),
							("F₁", [1. - v, 1. - v, 1. - ω]),
							("F₂", [v, v - 1., ω]),
							("H", [ζ, ζ, n]),
							("H₁", [1. - ζ, 1. - ζ, 1. - n]),
							("H₂", [-ζ, -ζ, 1. - n]),
							("I", [ρ, 1. - ρ, 0.5]),
							("I₁", [1. - ρ, ρ - 1., 0.5]),
							("L", [0.5, 0.5, 0.5]),
							("M", [0.5, 0., 0.5]),
							("N", [0.5, 0., 0.]),
							("N₁", [0., -0.5, 0.]),
							("X", [0.5, -0.5, 0.]),
							("Y", [μ, μ, δ]),
							("Y₁", [1. - μ, -μ, -δ]),
							("Y₂", [-μ, -μ, -δ]),
							("Y₃", [μ, μ - 1., δ]),
							("Z", [0., 0., 0.5]),
						])
					}
				}
			}
			| Orthorhombic(orc) => {
				use Orc::*;
				let a2 = params.a() * params.a();
				let b2 = params.b() * params.b();
				let c2 = params.c() * params.c();

				match orc {
					| ORC => KPoints::from([
						("Γ", [0., 0., 0.]),
						("R", [0.5, 0.5, 0.5]),
						("S", [0.5, 0.5, 0.]),
						("T", [0., 0.5, 0.5]),
						("U", [0.5, 0., 0.5]),
						("X", [0.5, 0., 0.]),
						("Y", [0., 0.5, 0.]),
						("Z", [0., 0., 0.5]),
					]),
					| ORCF1 | ORCF3 => {
						let ζ = 0.25 * (1. + a2 / b2 - a2 / c2);
						let n = 0.25 * (1. + a2 / b2 + a2 / c2);

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("A", [0.5, 0.5 + ζ, ζ]),
							("A₁", [0.5, 0.5 - ζ, 1. - ζ]),
							("L", [0.5, 0.5, 0.5]),
							("T", [1., 0.5, 0.5]),
							("X", [0., n, n]),
							("X₁", [1., 1. - n, 1. - n]),
							("Y", [0.5, 0., 0.5]),
							("Z", [0.5, 0.5, 0.]),
						])
					}
					| ORCF2 => {
						let n = 0.25 * (1. + a2 / b2 - a2 / c2);
						let φ = 0.25 * (1. + c2 / b2 - c2 / a2);
						let δ = 0.25 * (1. + b2 / a2 - b2 / c2);

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("C", [0.5, 0.5 - n, 1. - n]),
							("C₁", [0.5, 0.5 + n, n]),
							("D", [0.5 - δ, 0.5, 1. - δ]),
							("D₁", [0.5 + δ, 0.5, δ]),
							("L", [0.5, 0.5, 0.5]),
							("H", [1. - φ, 0.5 - φ, 0.5]),
							("H₁", [φ, 0.5 + φ, 0.5]),
							("X", [0., 0.5, 0.5]),
							("Y", [0.5, 0., 0.5]),
							("Z", [0.5, 0.5, 0.]),
						])
					}
					| ORCI => {
						let ζ = 0.25 * (1. + a2 / c2);
						let n = 0.25 * (1. + b2 / c2);
						let δ = (b2 - a2) / (4. * c2);
						let μ = (a2 + b2) / (4. * c2);

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("L", [-μ, μ, 0.5 - δ]),
							("L₁", [μ, -μ, 0.5 + δ]),
							("L₂", [0.5 - δ, 0.5 + δ, -μ]),
							("R", [0., 0.5, 0.]),
							("S", [0.5, 0., 0.]),
							("T", [0., 0., 0.5]),
							("W", [0.25, 0.25, 0.25]),
							("X", [-ζ, ζ, ζ]),
							("X₁", [ζ, 1. - ζ, -ζ]),
							("Y", [n, -n, n]),
							("Y₁", [1. - n, n, -n]),
							("Z", [0.5, 0.5, -0.5]),
						])
					}
					| ORCC => {
						let ζ = 0.25 * (1. + a2 / b2);

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("A", [ζ, ζ, 0.5]),
							("A₁", [-ζ, 1. - ζ, 0.5]),
							("R", [0., 0.5, 0.5]),
							("S", [0., 0.5, 0.]),
							("T", [-0.5, 0.5, 0.5]),
							("X", [ζ, ζ, 0.]),
							("X₁", [-ζ, 1. - ζ, 0.]),
							("Z", [0., 0., 0.5]),
						])
					}
				}
			}
			| Tetragonal(tet) => {
				use Tet::*;
				let a2 = params.a() * params.a();
				let c2 = params.c() * params.c();
				match tet {
					| TET => KPoints::from([
						("Γ", [0., 0., 0.]),
						("A", [0.5, 0.5, 0.5]),
						("M", [0.5, 0.5, 0.]),
						("R", [0., 0.5, 0.5]),
						("X", [0., 0.5, 0.]),
						("Z", [0., 0., 0.5]),
					]),
					| BCT1 => {
						let n = (1. + c2 / a2) / 4.;
						KPoints::from([
							("Γ", [0., 0., 0.]),
							("M", [-0.5, 0.5, 0.5]),
							("N", [0., 0.5, 0.]),
							("P", [0.25, 0.25, 0.25]),
							("X", [0., 0., 0.5]),
							("Z", [n, n, -n]),
							("Z1", [-n, 1. - n, n]),
						])
					}
					| BCT2 => {
						let n = (1. + a2 / c2) / 4.;
						let ζ = a2 / (2. * c2);
						KPoints::from([
							("Γ", [0., 0., 0.]),
							("N", [0., 0.5, 0.]),
							("P", [0.25, 0.25, 0.25]),
							("Σ", [-n, n, n]),
							("Σ₁", [n, 1. - n, -n]),
							("X", [0., 0., 0.5]),
							("Y", [-ζ, ζ, 0.5]),
							("Y₁", [0.5, 0.5, -ζ]),
							("Z", [0.5, 0.5, -0.5]),
						])
					}
				}
			}
			| Rhombohedral(rhl) => {
				use Rhl::*;

				let α = params.α();
				match rhl {
					| RHL1 => {
						let n = (1. + 4. * α.cos()) / (2. + 4. * α.cos());
						let v = 0.75 - 0.5 * n;

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("B", [n, 0.5, 1. - n]),
							("B₁", [0.5, 1. - n, n - 1.]),
							("F", [0.5, 0.5, 0.]),
							("L", [0.5, 0., 0.]),
							("L₁", [0., 0., -0.5]),
							("P", [n, v, v]),
							("P₁", [1. - v, 1. - v, 1. - n]),
							("Q", [1. - v, v, 0.]),
							("X", [v, 0., -v]),
							("Z", [0.5, 0.5, 0.5]),
						])
					}
					| RHL2 => {
						let n = 1. / (3. * (α * 0.5).tan().powf(2.));
						let v = 0.75 - 0.5 * n;

						KPoints::from([
							("Γ", [0., 0., 0.]),
							("F", [0.5, -0.5, 0.]),
							("L", [0.5, 0., 0.]),
							("P", [1. - v, -v, 1. - v]),
							("P₁", [v, v - 1., v - 1.]),
							("Q", [n, n, n]),
							("Q₁", [1. - n, -n, -n]),
							("Z", [0.5, -0.5, -0.5]),
						])
					}
				}
			}
			| Hexagonal => KPoints::from([
				("Γ", [0., 0., 0.]),
				("A", [0., 0., 0.5]),
				("H", [1. / 3., 1. / 3., 0.5]),
				("K", [1. / 3., 1. / 3., 0.]),
				("L", [0.5, 0., 0.5]),
				("M", [0.5, 0., 0.]),
			]),
			| Cubic(cub) => {
				use Cub::*;

				match cub {
					| CUB => KPoints::from([
						("Γ", [0., 0., 0.]),
						("M", [0.5, 0.5, 0.]),
						("R", [0.5, 0.5, 0.5]),
						("X", [0., 0.5, 0.]),
					]),
					| FCC => KPoints::from([
						("Γ", [0., 0., 0.]),
						("K", [0.3 / 0.8, 0.3 / 0.8, 0.75]),
						("L", [0.5, 0.5, 0.5]),
						("U", [0.5 / 0.8, 0.25, 0.5 / 0.8]),
					]),
					| BCC => KPoints::from([
						("Γ", [0., 0., 0.]),
						("H", [0.5, -0.5, 0.5]),
						("P", [0.25, 0.25, 0.25]),
						("N", [0., 0., 0.5]),
					]),
				}
			}
		}
	}
}

pub fn space_groups() -> Vec<String> {
	vec![
		"P1",
		"P1̅",
		"P2",
		"P2₁",
		"C2",
		"Pm",
		"Pc",
		"Cm",
		"Cc",
		"P2/m",
		"P2₁/m",
		"C2/m",
		"P2/c",
		"P2₁/c",
		"C2/c",
		"P222",
		"P222₁",
		"P2₁2₁2",
		"P2₁2₁2₁",
		"C222₁",
		"C222",
		"F222",
		"I222",
		"I2₁2₁2₁",
		"Pmm2",
		"Pmc2₁",
		"Pcc2",
		"Pma2",
		"Pca2₁",
		"Pnc2",
		"Pmn2₁",
		"Pba2",
		"Pna2₁",
		"Pnn2",
		"Cmm2",
		"Cmc2₁",
		"Ccc2",
		"Amm2",
		"Aem2",
		"Ama2",
		"Aea2",
		"Fmm2",
		"Fdd2",
		"Imm2",
		"Iba2",
		"Ima2",
		"Pmmm",
		"Pnnn",
		"Pccm",
		"Pban",
		"Pmma",
		"Pnna",
		"Pmna",
		"Pcca",
		"Pbam",
		"Pccn",
		"Pbcm",
		"Pnnm",
		"Pmmn",
		"Pbcn",
		"Pbca",
		"Pnma",
		"Cmcm",
		"Cmce",
		"Cmmm",
		"Cccm",
		"Cmme",
		"Ccce",
		"Fmmm",
		"Fddd",
		"Immm",
		"Ibam",
		"Ibca",
		"Imma",
		"P4",
		"P4₁",
		"P4₂",
		"P4₃",
		"I4",
		"I4₁",
		"P4̄",
		"I4̄",
		"P4/m",
		"P4₂/m",
		"P4/n",
		"P4₂/n",
		"I4/m",
		"I4₁/a",
		"P422",
		"P42₁2",
		"P4₁22",
		"P4₁2₁2",
		"P4₂22",
		"P4₂2₁2",
		"P4₃22",
		"P4₃2₁2",
		"I422",
		"I4₁22",
		"P4mm",
		"P4bm",
		"P4₂cm",
		"P4₂nm",
		"P4cc",
		"P4nc",
		"P4₂mc",
		"P4₂bc",
		"I4mm",
		"I4cm",
		"I4₁md",
		"I4₁cd",
		"P4̄2m",
		"P4̄2c",
		"P4̄2₁m",
		"P4̄2₁c",
		"P4̄m2",
		"P4̄c2",
		"P4̄b2",
		"P4̄n2",
		"I4̄m2",
		"I4̄c2",
		"I4̄2m",
		"I4̄2d",
		"P4/mmm",
		"P4/mcc",
		"P4/nbm",
		"P4/nnc",
		"P4/mbm",
		"P4/mnc",
		"P4/nmm",
		"P4/ncc",
		"P4₂/mmc",
		"P4₂/mcm",
		"P4₂/mcm",
		"P4₂/nbc",
		"P4₂/nnm",
		"P4₂/mbc",
		"P4₂/mnm",
		"P4₂/nmc",
		"P4₂/ncm",
		"I4/mmm",
		"I4/mcm",
		"I4₁/amd",
		"I4₁/acd",
		"P3",
		"P3₁",
		"P3₂",
		"R3",
		"P3̅",
		"R3̅",
		"P312",
		"P321",
		"P3₁12",
		"P3₁21",
		"P3₂12",
		"P3₂21",
		"R32",
		"P3m1",
		"P31m",
		"P3c1",
		"P31c",
		"R3m",
		"R3c",
		"P-3",
		"P-31c",
		"P-3m1",
		"P-3c1",
		"R-3m",
		"R-3c",
		"P6",
		"P6₁",
		"P6₅",
		"P6₂",
		"P6₄",
		"P6₃",
		"P-6",
		"P6/m",
		"P6₃/m",
		"P622",
		"P6₁22",
		"P6₅22",
		"P6₂22",
		"P6₄22",
		"P6₃22",
		"P6mm",
		"P6cc",
		"P6₃cm",
		"P6₃mc",
		"P-6m2",
		"P-6c2",
		"P-62m",
		"P-62c",
		"P6/mmm",
		"P6/mcc",
		"P6₃/mcm",
		"P6₃/mmc",
		"P23",
		"F23",
		"I23",
		"P2₁3",
		"I2₁3",
		"Pm-3",
		"Pn-3",
		"Fm-3",
		"Fd-3",
		"Im-3",
		"Pa-3",
		"Ia-3",
		"P432",
		"P4₂32",
		"F432",
		"F4₁32",
		"I432",
		"P4₃32",
		"P4₁32",
		"I4₁32",
		"P4̄3m",
		"F4̄3m",
		"I4̄3m",
		"P4̄3n",
		"F4̄3c",
		"I4̄3d",
		"Pm-3m",
		"Pn-3n",
		"Pm-3n",
		"Pn-3m",
		"Fm-3m",
		"Fm-3c",
		"Fd-3m",
		"Fd-3c",
		"Im-3m",
		"Ia-3d",
	]
	.iter()
	.map(|s| String::from(*s))
	.collect::<Vec<_>>()
}

pub type KPoints = HashMap<&'static str, [f64; 3]>;

pub struct KPoint {
	pub name:  String,
	pub coord: [f64; 3],
}

pub enum Tri {
	P1,
	P1Minus,
}

pub enum Tet {
	TET,
	BCT1,
	BCT2,
}

pub enum Mcl {
	MCL,
	MCLC1,
	MCLC2,
	MCLC3,
	MCLC4,
	MCLC5,
}

pub enum Orc {
	ORC,
	ORCF1,
	ORCF2,
	ORCF3,
	ORCI,
	ORCC,
}

pub enum Rhl {
	RHL1,
	RHL2,
}

pub enum Cub {
	CUB,
	FCC,
	BCC,
}

pub struct LatticeParameters([f64; 6]);

impl From<&[f64; 6]> for LatticeParameters {
	fn from(value: &[f64; 6]) -> Self {
		Self(*value)
	}
}

impl LatticeParameters {
	pub fn a(&self) -> f64 {
		self.0[0]
	}

	pub fn b(&self) -> f64 {
		self.0[1]
	}

	pub fn c(&self) -> f64 {
		self.0[2]
	}

	pub fn α(&self) -> f64 {
		self.0[3]
	}

	pub fn β(&self) -> f64 {
		self.0[4]
	}

	pub fn γ(&self) -> f64 {
		self.0[5]
	}
}
