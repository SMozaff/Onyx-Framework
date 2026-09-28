import { StrictMode, useEffect } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppRoutes } from './routes';
import { registerServiceWorker } from './lib/pwa';
import './styles.css';
import './i18n/rtl.css';
import { LocaleProvider } from './i18n/I18nContext';
import { dictionaries } from './i18n/dictionaries.generated';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      retry: 1,
      refetchOnWindowFocus: false,
      networkMode: 'always',
    },
  },
});

function App() {
  useEffect(() => {
    void registerServiceWorker();
  }, []);

  useEffect(() => {
    const onToast = (event: Event) => {
      const detail = (event as CustomEvent<{ message: string; tone: string }>).detail;
      console.info(`[toast:${detail.tone}] ${detail.message}`);
    };
    window.addEventListener('onyx:toast', onToast);
    return () => window.removeEventListener('onyx:toast', onToast);
  }, []);

  return (
    <StrictMode>
      <LocaleProvider dictionaries={dictionaries}>
        <QueryClientProvider client={queryClient}>
          <BrowserRouter>
            <AppRoutes />
          </BrowserRouter>
        </QueryClientProvider>
      </LocaleProvider>
    </StrictMode>
  );
}

createRoot(document.getElementById('root')!).render(<App />);