import React from "react";

/**
 * Lockup de marca de Dixi: el motivo de onda de voz junto al nombre.
 *
 * Sustituye al logotipo de Handy, que era un trazo vectorial de su nombre
 * (un recurso de marca de Handy, fuera de la cobertura de su licencia MIT).
 *
 * El nombre se compone con tipografia del sistema, no con un trazado: no hay
 * activo de marca proprietary, y el color lo hereda del tema a traves de las
 * clases `logo-primary` / `logo-stroke`, igual que el resto de la interfaz.
 */
const DixiTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  // Onda de 5 barras (mismo motivo que DixiMark) a la izquierda del nombre.
  const bars = [
    { x: 4, y: 30, h: 40 },
    { x: 20, y: 18, h: 64 },
    { x: 36, y: 4, h: 92 },
    { x: 52, y: 18, h: 64 },
    { x: 68, y: 30, h: 40 },
  ];

  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 300 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="Dixi"
    >
      {bars.map((b, i) => (
        <rect
          key={i}
          x={b.x}
          y={b.y}
          width={12}
          height={b.h}
          rx={6}
          ry={6}
          className="logo-primary"
        />
      ))}
      {/* The brand name is part of the logo, not a translatable string. */}
      {/* eslint-disable-next-line i18next/no-literal-string */}
      <text
        x="94"
        y="76"
        fontFamily="system-ui, 'Segoe UI', 'Helvetica Neue', Arial, sans-serif"
        fontSize="72"
        fontWeight={700}
        letterSpacing="-1.5"
        className="logo-primary"
      >
        Dixi
      </text>
    </svg>
  );
};

export default DixiTextLogo;
