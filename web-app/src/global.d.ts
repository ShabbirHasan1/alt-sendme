declare global {
  const IS_TAURI: boolean;
  const IS_MACOS: boolean;
  const IS_WINDOWS: boolean;
  const IS_LINUX: boolean;
  
  interface Window {
    goatcounter?: {
      count: (event: {
        event: string;
        data?: Record<string, any>;
      }) => void;
    };
  }
}

export {};

