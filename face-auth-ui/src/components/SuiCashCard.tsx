/**
 * SuiCash プリペイドカードの券面。assets/card/suicash-card.svg と同一デザイン。
 * - 地はシルバー、左にテーマカラー(アクア #4DA2FF 系)の台形パネル
 * - ワードマークはパネル内左下、assets/logo/suicash-logo.svg と同じ組み方(REM 605、iC のみアウトライン)
 * - マスコットは assets/mascot/suicash-mascot.svg をインライン縮小配置
 *   (外部 SVG を <img> で読むと Web フォントが遮断されるため全てインラインで組む)
 */
export function SuiCashCard({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 172 108"
      className={className}
      role="img"
      aria-label="SuiCash カード"
    >
      <defs>
        <linearGradient id="scSilver" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#f8f9f8" />
          <stop offset="1" stopColor="#e2e6e4" />
        </linearGradient>
        <linearGradient id="scAqua" x1="0" y1="0" x2="0.5" y2="1">
          <stop offset="0" stopColor="#5cb0ff" />
          <stop offset="1" stopColor="#3d95f5" />
        </linearGradient>
        <clipPath id="scCardClip">
          <rect x="0" y="0" width="172" height="108" rx="9" />
        </clipPath>
      </defs>

      <g clipPath="url(#scCardClip)">
        {/* 下地(シルバー) */}
        <rect x="0" y="0" width="172" height="108" fill="url(#scSilver)" />

        {/* 台形パネル(テーマカラーのアクア) */}
        <path
          d="M 12 7 H 76 Q 80 7 81.2 10.8 L 108.8 98.2 Q 110 102 106 102 H 12 Q 7 102 7 97 V 12 Q 7 7 12 7 Z"
          fill="url(#scAqua)"
        />

        {/* 左上の挿入方向マーク */}
        <path d="M 15 2.2 L 10.4 4.3 L 15 6.4 Z" fill="#0a1a2f" opacity="0.7" />

        {/* マスコット(assets/mascot/suicash-mascot.svg を埋め込み) */}
        <g transform="translate(110 36.4) scale(0.113)">
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

        {/* ワードマーク: 台形パネル内の左下に白で(iC はアウトライン) */}
        <text
          x="12"
          y="94"
          fontFamily="REM, sans-serif"
          fontWeight="605"
          fontSize="22"
          style={{
            fontVariationSettings: "'wght' 605",
            fontVariantLigatures: "none",
            fontKerning: "none",
          }}
        >
          <tspan fill="#ffffff">Su</tspan>
          <tspan fill="none" stroke="#ffffff" strokeWidth="1" strokeLinejoin="miter">
            iC
          </tspan>
          <tspan fill="#ffffff">ash</tspan>
        </text>
      </g>

      <rect
        x="0.75"
        y="0.75"
        width="170.5"
        height="106.5"
        rx="8.5"
        fill="none"
        stroke="rgba(10, 26, 47, 0.28)"
        strokeWidth="1.5"
      />
    </svg>
  );
}
