import styled from 'styled-components';
import { color, border, radius, shadow, space } from '@/theme';

/**
 * Standard white surface card. Replaces ad-hoc card styling across
 * HeaderCard, InstrumentCard, MetricCard body, KLine bottom panels,
 * task-detail body cards.
 *
 * Padding default: 12px / 16px. Override via inline `style`/`className`
 * or wrap in another styled component if a page needs different padding.
 */
const Card = styled.div`
  background: ${color.bgSurface};
  border: ${border.default};
  border-radius: ${radius.card};
  box-shadow: ${shadow.card};
  padding: ${space.px12}px ${space.px16}px;
`;

export default Card;
