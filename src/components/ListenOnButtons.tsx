import { useState } from "react";

interface Props {
  feedUrl: string;
  onCopyRss: () => void;
  rssCopied: boolean;
}

interface AppButton {
  name: string;
  href: string;
  iconUrl: string;
}

const CDN = "https://d12xoj7p9moygp.cloudfront.net/images/podcast/logo-square/006";
const SITE = "https://casti.fyi";

function buildButtons(feedUrl: string): AppButton[] {
  const enc = encodeURIComponent(feedUrl);
  const noProto = feedUrl.replace(/^https?:\/\//, "");
  return [
    { name: "Apple Podcasts", href: `podcast://${noProto}`, iconUrl: `${CDN}/apple_podcasts@2x.png` },
    { name: "Pocket Casts", href: `pktc://subscribe/${feedUrl}`, iconUrl: `${CDN}/pocket_casts@2x.png` },
    { name: "Overcast", href: `overcast:///add?url=${enc}`, iconUrl: `${CDN}/overcast@2x.png` },
    { name: "Castro", href: `castro://subscribe/${enc}`, iconUrl: `${SITE}/images/icon_castro.png` },
    { name: "AntennaPod", href: `antennapod-subscribe://${feedUrl}`, iconUrl: `${SITE}/images/icon_antennapod.png` },
    { name: "Podcast Addict", href: `podcastaddict://subscribe/${enc}`, iconUrl: `${SITE}/images/icon_podcastaddict.png` },
  ];
}

export default function ListenOnButtons({ feedUrl, onCopyRss, rssCopied }: Props) {
  const [qrApp, setQrApp] = useState<AppButton | null>(null);
  const buttons = buildButtons(feedUrl);

  return (
    <>
      <div className="listen-on-section">
        <span className="listen-on-label">Listen on</span>
        <div className="listen-on-buttons">
          {buttons.map((b) => (
            <button
              key={b.name}
              className="listen-on-icon"
              title={b.name}
              onClick={() => setQrApp(b)}
            >
              <img src={b.iconUrl} alt={b.name} width={28} height={28} />
            </button>
          ))}
          <span className="listen-on-rss-wrap">
            <button
              className="listen-on-icon"
              onClick={onCopyRss}
              title="Copy RSS URL"
            >
              <svg width="28" height="28" viewBox="0 0 28 28">
                <rect fill="#FE8A4C" width="28" height="28" rx="6"/>
                <path d="M6.822 18.536c.741 0 1.367.265 1.878.793.519.514.778 1.14.778 1.877 0 .737-.26 1.367-.778 1.888-.511.514-1.137.771-1.878.771-.733 0-1.355-.257-1.866-.771-.511-.521-.767-1.15-.767-1.888 0-.737.256-1.363.767-1.877.511-.529 1.133-.793 1.866-.793zm-2.544-6.983c.17-.194.385-.306.644-.335v-.011h1.089c2.97 0 5.511 1.057 7.622 3.173 2.111 2.1 3.178 4.644 3.2 7.631v1.061h-.011c-.022.238-.119.443-.29.615-.17.171-.377.272-.621.302v.011H14.055c-.26-.007-.486-.097-.678-.268-.193-.186-.304-.406-.334-.66h-.011v-1.06h.011c-.022-1.93-.715-3.58-2.078-4.95-1.385-1.363-3.037-2.045-4.956-2.045h-.043v.012H4.922v-.012c-.237-.03-.445-.126-.622-.29-.171-.171-.263-.38-.278-.626H4v-1.866c.015-.26.107-.488.278-.682zm0-4.66V5.017c.015-.261.107-.488.278-.682.17-.193.385-.305.644-.335h1.089C10.952 4 15.181 5.758 18.7 9.274 22.218 12.797 23.985 17.042 24 22.011v1.061h-.011c-.03.238-.126.443-.29.615-.17.171-.377.272-.621.302V24h-1.856c-.26-.008-.485-.097-.678-.268-.193-.186-.304-.406-.333-.66h-.012v-1.06h.012c-.015-3.919-1.411-7.263-4.189-10.034-2.77-2.786-6.108-4.179-10.011-4.179h-.044v.012H4.922v-.012c-.237-.022-.445-.119-.622-.29-.171-.172-.263-.377-.278-.615H4z" fill="#FFF" fillRule="nonzero"/>
              </svg>
            </button>
            {rssCopied && <span className="listen-on-copied">Copied!</span>}
          </span>
        </div>
      </div>

      {qrApp && (
        <div className="qr-overlay" onClick={() => setQrApp(null)}>
          <div className="qr-modal" onClick={(e) => e.stopPropagation()}>
            <button className="qr-modal-close" onClick={() => setQrApp(null)}>
              &times;
            </button>
            <img
              src={qrApp.iconUrl}
              alt={qrApp.name}
              width={40}
              height={40}
              style={{ borderRadius: 10, marginBottom: 8 }}
            />
            <h4>{qrApp.name}</h4>
            <p className="qr-modal-hint">Scan with your phone to subscribe</p>
            <img
              src={`https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${encodeURIComponent(qrApp.href)}`}
              alt="QR Code"
              width={200}
              height={200}
              style={{ borderRadius: 8 }}
            />
          </div>
        </div>
      )}
    </>
  );
}
