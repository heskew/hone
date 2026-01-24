import { Cpu } from "lucide-react";

interface FooterProps {
  onNavigateToAIMetrics: () => void;
}

export function Footer({ onNavigateToAIMetrics }: FooterProps) {
  const version = __BUILD_VERSION__;

  return (
    <footer className="border-t border-hone-100 dark:border-hone-700 bg-white dark:bg-hone-900 mt-auto">
      <div className="max-w-[1800px] mx-auto px-4 sm:px-6 lg:px-8 py-4">
        <div className="flex items-center justify-between text-sm text-hone-400">
          <div className="flex items-center gap-4">
            <span>Hone v{version}</span>
            <button
              onClick={onNavigateToAIMetrics}
              className="flex items-center gap-1 hover:text-hone-600 dark:hover:text-hone-300 transition-colors"
            >
              <Cpu className="w-3.5 h-3.5" />
              <span>AI Metrics</span>
            </button>
          </div>
          <div>
            <a
              href="https://github.com/heskew/hone"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-hone-600 dark:hover:text-hone-300 transition-colors"
            >
              GitHub
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
}
