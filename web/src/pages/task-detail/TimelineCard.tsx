import styled from 'styled-components';
import { color, font, space } from '@/theme';

interface Props {
  icon: string;
  bgColor: string;
  iconColor: string;
  lineColor?: string;
  isLast?: boolean;
  children: React.ReactNode;
}

export default function TimelineCard({
  icon,
  bgColor,
  iconColor,
  lineColor,
  isLast,
  children,
}: Props) {
  return (
    <Row>
      <TimelineCol>
        <Node style={{ background: bgColor, color: iconColor }}>{icon}</Node>
        {!isLast && <Line style={{ background: lineColor ?? color.textDark }} />}
      </TimelineCol>
      <Content>{children}</Content>
    </Row>
  );
}

/* ---- styled ---- */

const Row = styled.div`
  display: flex;
  gap: ${space.px12}px;
`;

const TimelineCol = styled.div`
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 48px;
  flex-shrink: 0;
`;

const Node = styled.div`
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: ${font.mono};
  font-size: 16px;
  font-weight: 700;
  flex-shrink: 0;
`;

const Line = styled.div`
  width: 2px;
  flex: 1;
  min-height: 12px;
`;

const Content = styled.div`
  flex: 1;
  padding-bottom: ${space.px16}px;
  min-width: 0;
`;
