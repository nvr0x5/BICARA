/** Small line icons in the SF Symbols / Fluent style (16px grid, currentColor). */
type P = { className?: string };
const S = ({ children, className }: P & { children: React.ReactNode }) => (
  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={className} aria-hidden="true">
    {children}
  </svg>
);
export const IPlus = (p: P) => <S {...p}><path d="M8 3v10M3 8h10" /></S>;
export const IWave = (p: P) => <S {...p}><path d="M2 7v2M5 4.5v7M8 2v12M11 4.5v7M14 7v2" /></S>;
export const IDoc = (p: P) => <S {...p}><path d="M4 1.5h5l3 3v10H4z M9 1.5v3h3" /></S>;
export const IClock = (p: P) => <S {...p}><circle cx="8" cy="8" r="6" /><path d="M8 4.5V8l2.5 1.5" /></S>;
/** Cog wheel (settings). Drawn on a 24px grid for smoother teeth. */
export const IGear = ({ className }: P) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" className={className} aria-hidden="true">
    <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
    <circle cx="12" cy="12" r="3" />
  </svg>
);
export const IEyeOff = (p: P) => <S {...p}><path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" /><path d="M3 13L13 3" /></S>;
export const IStop = (p: P) => <S {...p}><rect x="3.5" y="3.5" width="9" height="9" rx="2" fill="currentColor" stroke="none" /></S>;
export const ISidebar = (p: P) => <S {...p}><rect x="1.5" y="2.5" width="13" height="11" rx="2" /><path d="M6 2.5v11" /></S>;
export const IInspector = (p: P) => <S {...p}><rect x="1.5" y="2.5" width="13" height="11" rx="2" /><path d="M10 2.5v11" /></S>;
export const IShare = (p: P) => <S {...p}><path d="M8 10V2M5 4.5L8 1.5l3 3M4 7H3v7.5h10V7h-1" /></S>;
export const ITrash = (p: P) => <S {...p}><path d="M2.5 4h11M6 4V2.5h4V4M4 4l.7 10h6.6L12 4" /></S>;
export const IMic = (p: P) => <S {...p}><rect x="5.5" y="1.5" width="5" height="8" rx="2.5" /><path d="M3.5 7.5a4.5 4.5 0 009 0M8 12v2.5" /></S>;
/** AI sparkles: a large four-point star with a small one, filled. */
export const ISpark = ({ className }: P) => (
  <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
    <path d="M10 5.5c.75 4.4 3.1 6.75 7.5 7.5-4.4.75-6.75 3.1-7.5 7.5-.75-4.4-3.1-6.75-7.5-7.5 4.4-.75 6.75-3.1 7.5-7.5z" />
    <path d="M18 2.5c.35 2 1.35 3 3.35 3.35-2 .35-3 1.35-3.35 3.35-.35-2-1.35-3-3.35-3.35 2-.35 3-1.35 3.35-3.35z" />
  </svg>
);
export const ILock = (p: P) => <S {...p}><rect x="3" y="7" width="10" height="7" rx="1.5" /><path d="M5.5 7V5a2.5 2.5 0 015 0v2" /></S>;
export const IGlobe = (p: P) => <S {...p}><circle cx="8" cy="8" r="6" /><path d="M2 8h12M8 2c2 2 2 10 0 12M8 2c-2 2-2 10 0 12" /></S>;
export const IList = (p: P) => <S {...p}><path d="M5.5 4h8M5.5 8h8M5.5 12h8M2.5 4h.5M2.5 8h.5M2.5 12h.5" /></S>;
export const IPencil = (p: P) => <S {...p}><path d="M10.5 2.5l3 3L6 13H3v-3z" /></S>;
export const IRefresh = (p: P) => <S {...p}><path d="M13 3v3.5H9.5M3 13V9.5h3.5" /><path d="M12.6 6.5A5 5 0 003.6 5M3.4 9.5a5 5 0 009 1.5" /></S>;
export const IMicOff = (p: P) => <S {...p}><path d="M10.5 6.5V4a2.5 2.5 0 00-4.9-.7M5.5 6v1.5a2.5 2.5 0 004 2M3.5 7.5a4.5 4.5 0 007.3 3.5M12.4 8.6a4.5 4.5 0 00.1-1.1M8 12v2.5M2 2l12 12" /></S>;
