import { createRoot } from 'react-dom/client';
import App from './App';
import './styles.css';
import './design.css';

// В лаунчере не нужно браузерное меню по правой кнопке.
document.addEventListener('contextmenu', (e) => e.preventDefault());

const root = document.getElementById('root');
if (root) createRoot(root).render(<App />);
