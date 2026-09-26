/**
 * SuiCash prepaid card face. Original design in the style of a transit IC card.
 * - Silver background; the polygonal mountains at the bottom use the theme color (aqua, #4DA2FF family)
 * - Wordmark set the same way as assets/logo/suicash-logo.svg (REM 605, only iC outlined)
 * - Mascot is assets/mascot/suicash-mascot.svg inlined and scaled down
 *   (loading external SVG via <img> blocks web fonts, so everything is inlined)
 */
export function SuiCashCard({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 172 108"
      className={className}
      role="img"
      aria-label="SuiCash card"
    >
      <defs>
        <linearGradient id="scCardBody" x1="0" y1="0" x2="0.4" y2="1">
          <stop offset="0" stopColor="#f7fafc" />
          <stop offset="0.55" stopColor="#e3e9ef" />
          <stop offset="1" stopColor="#cdd6de" />
        </linearGradient>
        <linearGradient id="scChip" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#e8c96a" />
          <stop offset="1" stopColor="#c9a13e" />
        </linearGradient>
        <clipPath id="scCardClip">
          <rect x="0" y="0" width="172" height="108" rx="9" />
        </clipPath>
      </defs>

      <g clipPath="url(#scCardClip)">
        <rect x="0" y="0" width="172" height="108" fill="url(#scCardBody)" />

        {/* Polygonal mountain range at the bottom, faceted in the aqua theme color */}
        <g>
          <polygon
            points="0,70 30,52 60,66 94,46 126,62 172,42 172,108 0,108"
            fill="#4da2ff"
          />
          <polygon points="30,52 60,66 22,108 0,108 0,70" fill="#2f6fb8" />
          <polygon points="94,46 126,62 96,108 56,108" fill="#6cb4ff" />
          <polygon points="126,62 172,42 172,108 140,108" fill="#2f6fb8" />
          {/* White wave across the mountains */}
          <path
            d="M-4,82 C30,72 62,92 98,78 C126,68 152,80 176,72"
            fill="none"
            stroke="#ffffff"
            strokeWidth="3.4"
            strokeLinecap="round"
            opacity="0.92"
          />
        </g>

        {/* IC chip */}
        <g>
          <rect x="14" y="38" width="21" height="16" rx="2.5" fill="url(#scChip)" />
          <path
            d="M14 44 h21 M14 49 h21 M21 38 v16 M28 38 v16"
            stroke="#8f742c"
            strokeWidth="0.8"
            fill="none"
          />
        </g>

        {/* Wordmark: Su / iC (outlined) / ash */}
        <text
          x="13"
          y="27"
          fontFamily="REM, sans-serif"
          fontWeight="605"
          fontSize="20"
          style={{
            fontVariationSettings: "'wght' 605",
            fontVariantLigatures: "none",
            fontKerning: "none",
          }}
        >
          <tspan fill="#0a1a2f">Su</tspan>
          <tspan fill="#ffffff" stroke="#0a1a2f" strokeWidth="0.9" strokeLinejoin="miter">
            iC
          </tspan>
          <tspan fill="#0a1a2f">ash</tspan>
        </text>

        {/* Mascot standing on the mountains */}
        <g transform="translate(118 26) scale(0.082)">
          <g fill="none" strokeLinecap="round">
            <path d="M198 206 C180 150 162 92 158 46" stroke="#0A1A2F" strokeWidth="78" />
            <path d="M318 204 C340 164 366 130 396 112" stroke="#0A1A2F" strokeWidth="62" />
            <path d="M198 206 C180 150 162 92 158 46" stroke="#4DA2FF" strokeWidth="64" />
            <path d="M318 204 C340 164 366 130 396 112" stroke="#4DA2FF" strokeWidth="48" />
          </g>
          <g fill="none" strokeLinecap="round">
            <path d="M148 330 C142 382 138 414 137 442" stroke="#0A1A2F" strokeWidth="50" />
            <path d="M222 330 C219 372 216 404 215 428" stroke="#0A1A2F" strokeWidth="44" />
            <path d="M296 330 C300 368 302 398 303 418" stroke="#0A1A2F" strokeWidth="44" />
            <path d="M364 330 C370 382 374 414 375 442" stroke="#0A1A2F" strokeWidth="50" />
            <path d="M137 438 L104 482 M137 438 L137 492 M137 438 L170 482" stroke="#0A1A2F" strokeWidth="36" />
            <path d="M375 438 L342 482 M375 438 L375 492 M375 438 L408 482" stroke="#0A1A2F" strokeWidth="36" />
            <path d="M148 330 C142 382 138 414 137 442" stroke="#4DA2FF" strokeWidth="38" />
            <path d="M222 330 C219 372 216 404 215 428" stroke="#4DA2FF" strokeWidth="32" />
            <path d="M296 330 C300 368 302 398 303 418" stroke="#4DA2FF" strokeWidth="32" />
            <path d="M364 330 C370 382 374 414 375 442" stroke="#4DA2FF" strokeWidth="38" />
            <path d="M137 438 L104 482 M137 438 L137 492 M137 438 L170 482" stroke="#4DA2FF" strokeWidth="24" />
            <path d="M375 438 L342 482 M375 438 L375 492 M375 438 L408 482" stroke="#4DA2FF" strokeWidth="24" />
          </g>
          <g stroke="#0A1A2F" strokeWidth="10" strokeLinejoin="round">
            <path
              fill="#4DA2FF"
              d="M272 124 C358 128 424 174 432 246 C438 304 402 348 342 366 C276 386 134 392 100 354 C66 316 78 256 108 214 C144 164 202 122 272 124 Z"
            />
            <path
              fill="#FFFFFF"
              d="M236 250 C284 248 312 270 314 302 C316 334 294 356 252 360 C208 364 174 348 170 318 C166 286 190 252 236 250 Z"
            />
          </g>
          <g>
            <circle cx="215" cy="292" r="9.5" fill="#0A1A2F" />
            <circle cx="263" cy="288" r="8" fill="#0A1A2F" />
            <circle cx="218" cy="289" r="3.2" fill="#FFFFFF" />
            <circle cx="266" cy="285" r="2.7" fill="#FFFFFF" />
            <path d="M239 297 L251 313 L227 313 Z" fill="#0A1A2F" />
            <path d="M176 322 C202 372 290 368 312 316 C288 346 198 350 176 322 Z" fill="#0A1A2F" />
            <path d="M228 348 C238 364 258 362 266 346 Z" fill="#8FD6E8" />
          </g>
        </g>
      </g>

      <rect
        x="0.75"
        y="0.75"
        width="170.5"
        height="106.5"
        rx="8.5"
        fill="none"
        stroke="rgba(10, 26, 47, 0.25)"
        strokeWidth="1.5"
      />
    </svg>
  );
}
