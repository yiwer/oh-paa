import styled from 'styled-components';
import { color, font, border, radius, shadow, space, transition } from '@/theme';

export interface SegmentedOption<V extends string = string> {
  value: V;
  label: string;
}

interface Props<V extends string = string> {
  options: SegmentedOption<V>[];
  value: V;
  onChange: (v: V) => void;
  /** 'mono' for K-Line timeframe-style numeric labels; 'ui' for filter-style word labels. Default 'ui'. */
  variant?: 'mono' | 'ui';
}

export default function Segmented<V extends string = string>({
  options,
  value,
  onChange,
  variant = 'ui',
}: Props<V>) {
  return (
    <Container>
      {options.map((opt) => (
        <Item
          key={opt.value}
          type="button"
          $active={value === opt.value}
          $variant={variant}
          onClick={() => onChange(opt.value)}
        >
          {opt.label}
        </Item>
      ))}
    </Container>
  );
}

const Container = styled.div`
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.control};
  box-shadow: ${shadow.card};
`;

const Item = styled.button<{ $active: boolean; $variant: 'mono' | 'ui' }>`
  all: unset;
  cursor: pointer;
  padding: 5px ${space.px10}px;
  border-radius: 4px;
  font-family: ${(p) => (p.$variant === 'mono' ? font.mono : font.ui)};
  font-size: 11px;
  font-weight: 600;
  color: ${(p) => (p.$active ? color.text1 : color.text2)};
  background: ${(p) => (p.$active ? color.yellow : 'transparent')};
  transition: ${transition.control};

  &:hover {
    color: ${color.text1};
    background: ${(p) => (p.$active ? color.yellow : color.bgSoft)};
  }
`;
