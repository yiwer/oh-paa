/**
 * Warm MotherDuck design tokens.
 * Spec: docs/superpowers/specs/2026-04-27-web-ui-redesign-design.md
 *
 * Legacy aliases (bgBeige, bgWhite, tealAccent, redAccent, bluePrimary,
 * textDark, textGray, textLightGray, darkSurface, bgLightGray) are kept
 * pointing at the closest new value so Phase 1 doesn't break per-component
 * styles. Phase 2/3 migrate call sites to the new names.
 */

export const color = {
  /* === New canonical names === */
  bgPage: '#FAF7F2',
  bgSide: '#F1ECE2',
  bgSurface: '#FFFFFF',
  bgSoft: '#F7F2E8',

  borderHairline: '#E8E1D5',
  borderSoft: '#F0EAE0',

  text1: '#1A1A1A',
  text2: '#6B6B6B',
  text3: '#8A8580',
  textDisabled: '#C9BFA8',

  yellow: '#FFDE00',
  yellowSoft: '#FFF4D9',

  teal: '#2FB89A',
  tealSoft: '#E5F4EE',
  tealText: '#1B7A55',

  blue: '#4F8FE8',
  blueSoft: '#E8F0FB',
  blueText: '#2E62B8',

  red: '#E5685B',
  redSoft: '#FBE7E4',
  redText: '#B23F33',

  amberText: '#8A6500',

  /* === Legacy aliases (deprecated; do not use in new code) === */
  yellowPrimary: '#FFDE00',
  bluePrimary: '#4F8FE8',
  tealAccent: '#2FB89A',
  redAccent: '#E5685B',
  bgBeige: '#FAF7F2',
  bgOffwhite: '#F7F2E8',
  bgWhite: '#FFFFFF',
  bgLightGray: '#F7F2E8',
  textDark: '#1A1A1A',
  textGray: '#6B6B6B',
  textLightGray: '#8A8580',
  darkSurface: '#F1ECE2',
} as const;

export const font = {
  ui: '"Inter", "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  mono: '"JetBrains Mono", "PingFang SC", "Microsoft YaHei", monospace',
} as const;

export const size = {
  display: 32,
  h1: 24,
  h2: 18,
  h3: 14,
  body: 14,
  bodyLg: 16,
  bodySm: 13,
  bodyXs: 12,
  caption: 11,
  eyebrow: 10,
  mini: 11,
} as const;

export const space = {
  px4: 4, px6: 6, px8: 8, px10: 10, px12: 12,
  px16: 16, px20: 20, px24: 24, px32: 32, px48: 48,
} as const;

export const radius = {
  card: '10px',
  control: '6px',
  tag: '4px',
  pill: '999px',
} as const;

export const border = {
  default: `1px solid ${color.borderHairline}`,
  strong: `2px solid ${color.text1}`,
  dashed: `1px dashed ${color.borderSoft}`,
  dashedSection: `1px dashed ${color.borderSoft}`,
  soft: `1px solid ${color.borderSoft}`,

  /* legacy aliases */
  thin: `1px solid ${color.borderHairline}`,
  std: `1px solid ${color.borderHairline}`,
  thick: `2px solid ${color.text1}`,
  radius: '0px', // legacy; new code uses `radius.*`
} as const;

export const shadow = {
  card: '0 1px 2px rgba(28,25,20,.04)',
  hover: '0 4px 12px rgba(28,25,20,.06)',
  popover: '0 6px 20px rgba(28,25,20,.08)',
} as const;

export const transition = {
  control: 'background-color .15s ease, color .15s ease, border-color .15s ease',
  hover: 'transform .12s ease-in-out, box-shadow .15s ease',

  /* legacy aliases */
  btn: 'transform 0.12s ease-in-out',
  card: '0.4s ease-out',
  nav: 'background-color 0.2s ease-in-out',
} as const;
