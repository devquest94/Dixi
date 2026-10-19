/**
 * Marca de Dixi.
 *
 * Sustituye al glifo de Dixi (una mano), que es un recurso de marca de
 * Dixi y no forma parte de su licencia MIT.
 *
 * El motivo es una onda de voz simetrica de 5 barras: un indicador de nivel
 * de microfono. Es coherente con los iconos de bandeja de Dixi, que
 * comparten el mismo motivo.
 *
 * Se dibuja monocromo con las clases de tema (`fill-text stroke-text`) para
 * que herede el color del texto en los temas claro y oscuro.
 */
const DixiMark = ({
  width,
  height,
  className,
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
}) => {
  // viewBox 0 0 126 126. Alturas de barra relativas a un lienzo de 126.
  const bars = [
    { x: 12, y: 48, w: 15, h: 30 },
    { x: 34, y: 33, w: 15, h: 60 },
    { x: 56, y: 13, w: 15, h: 100 },
    { x: 78, y: 33, w: 15, h: 60 },
    { x: 100, y: 48, w: 15, h: 30 },
  ];

  return (
    <svg
      width={width || 126}
      height={height || 126}
      viewBox="0 0 126 126"
      className={className || "fill-text stroke-text"}
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="Dixi"
    >
      {bars.map((b, i) => (
        <rect
          key={i}
          x={b.x}
          y={b.y}
          width={b.w}
          height={b.h}
          rx={b.w / 2}
          ry={b.w / 2}
          fill="currentColor"
          stroke="none"
        />
      ))}
    </svg>
  );
};

export default DixiMark;
