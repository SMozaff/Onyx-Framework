import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App';
import './styles.css';
import './i18n/rtl.css';
import { LocaleProvider } from './i18n/I18nContext';
import { dictionaries } from './i18n/dictionaries.generated';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 60_000, retry: 1, refetchOnWindowFocus: false, networkMode: 'always' },
    mutations: { retry: false, networkMode: 'always' },
  },
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <LocaleProvider dictionaries={dictionaries}>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter><App /></BrowserRouter>
      </QueryClientProvider>
    </LocaleProvider>
  </React.StrictMode>,
);
