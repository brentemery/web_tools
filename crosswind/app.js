import init, { calculate_crosswind, calculate_headwind } from './crosswind-wasm/pkg/crosswind_wasm.js';

await init();

document.getElementById('calc-form').addEventListener('submit', function (e) {
  e.preventDefault();

  const runway = Number(document.getElementById('runway').value);
  const windDir = Number(document.getElementById('wind-dir').value);
  const windSpeed = Number(document.getElementById('wind-speed').value);

  const crosswind = calculate_crosswind(runway, windDir, windSpeed);
  const headwind = calculate_headwind(runway, windDir, windSpeed);

  document.getElementById('crosswind-value').textContent = crosswind;
  document.getElementById('headwind-value').textContent = headwind;

  const result = document.getElementById('result');
  result.hidden = false;

  // Negative headwind = tailwind
  const headwindLabel = result.querySelectorAll('p')[1];
  if (headwind < 0) {
    headwindLabel.innerHTML = 'Tailwind: <strong id="headwind-value">' + Math.abs(headwind) + '</strong> knots';
  } else {
    headwindLabel.innerHTML = 'Headwind: <strong id="headwind-value">' + headwind + '</strong> knots';
  }
});
