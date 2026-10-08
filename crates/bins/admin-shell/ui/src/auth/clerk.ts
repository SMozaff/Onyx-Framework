const PUBLISHABLE_KEY =
  import.meta.env.VITE_CLERK_PUBLISHABLE_KEY ||
  "pk_test_c3VwZXItbGlvbmZpc2gtMTc3NS5jbGVyay5hY2NvdW50cy5kZXYk";

type ClerkSession = { getToken: () => Promise<string | null> };
type ClerkInstance = {
  load: () => Promise<void>;
  session: ClerkSession | null;
  mountSignIn: (element: HTMLElement, options?: Record<string, unknown>) => void;
  unmountSignIn: (element: HTMLElement) => void;
  addListener: (listener: (resources: { session: ClerkSession | null }) => void) => () => void;
  signOut: () => Promise<void>;
};

let clerkPromise: Promise<ClerkInstance> | null = null;

export function loadClerk(): Promise<ClerkInstance> {
  if (clerkPromise) return clerkPromise;
  clerkPromise = new Promise((resolve, reject) => {
    const existing = (window as Window & { Clerk?: new (key: string) => ClerkInstance }).Clerk;
    if (existing) {
      const clerk = new existing(PUBLISHABLE_KEY);
      void clerk.load().then(() => resolve(clerk)).catch(reject);
      return;
    }
    const script = document.createElement("script");
    const encoded = PUBLISHABLE_KEY.split("_")[2];
    if (!encoded) {
      reject(new Error("Invalid Clerk publishable key"));
      return;
    }
    const domain = atob(encoded).replace(/\$$/, "");
    script.src = `https://${domain}/npm/@clerk/clerk-js@latest/dist/clerk.browser.js`;
    script.async = true;
    script.crossOrigin = "anonymous";
    script.onload = () => {
      const Clerk = (window as Window & { Clerk?: new (key: string) => ClerkInstance }).Clerk;
      if (!Clerk) {
        reject(new Error("Clerk SDK failed to initialize"));
        return;
      }
      const clerk = new Clerk(PUBLISHABLE_KEY);
      void clerk.load().then(() => resolve(clerk)).catch(reject);
    };
    script.onerror = () => reject(new Error("Unable to load Clerk"));
    document.head.appendChild(script);
  });
  return clerkPromise;
}
