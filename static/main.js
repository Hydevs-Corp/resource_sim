const canvas = document.getElementById('sim-canvas');
const ctx = canvas.getContext('2d');

const CELL_SIZE = 10;
let simWidth = 0;
let simHeight = 0;

const statusIndicator = document.getElementById('connection-status');
const statusText = statusIndicator.querySelector('.status-text');
const statBaseHp = document.getElementById('stat-base-hp');
const statCrystals = document.getElementById('stat-crystals');
const statMetal = document.getElementById('stat-metal');
const statMeat = document.getElementById('stat-meat');
const infoFear = document.getElementById('info-fear');
const infoRobots = document.getElementById('info-robots');
const infoEnemies = document.getElementById('info-enemies');
const infoWall = document.getElementById('info-wall');

const colors = {
    base: '#3b82f6',
    wall: '#94a3b8',
    door: '#cbd5e1',
    scout: '#2dd4bf',
    collector: '#f59e0b',
    army: '#ef4444',
    enemy: '#ec4899',
    crystal: '#60a5fa',
    metal: '#9ca3af',
    meat: '#f87171',
    energy: '#fbbf24',
    obstacle: '#334155',
    empty: '#020617',
    meteorite: '#d946ef'
};

function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    const ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        statusIndicator.classList.remove('offline');
        statusIndicator.classList.add('online');
        statusText.textContent = 'Connected';
    };

    ws.onclose = () => {
        statusIndicator.classList.remove('online');
        statusIndicator.classList.add('offline');
        statusText.textContent = 'Disconnected - Retrying...';
        setTimeout(connect, 2000);
    };

    ws.onmessage = (event) => {
        const state = JSON.parse(event.data);
        render(state);
        updateUI(state);
    };
}

function updateUI(state) {
    statBaseHp.textContent = state.base_hp;
    statCrystals.textContent = state.collected_crystals;
    statMetal.textContent = state.collected_metal;
    statMeat.textContent = state.collected_meat;

    infoFear.textContent = Math.round(state.fear_factor);
    infoRobots.textContent = state.robots.length;
    infoEnemies.textContent = state.enemies.length;
    infoWall.textContent = state.wall_built ? 'Yes' : 'No';
}

function getCellColor(cell) {
    if (typeof cell === 'string') {
        if (cell === 'Empty') return colors.empty;
        if (cell === 'Obstacle') return colors.obstacle;
        if (cell === 'Base') return colors.base;
    } else if (typeof cell === 'object') {
        if (cell.Wall !== undefined) return colors.wall;
        if (cell.Door !== undefined) return colors.door;
        if (cell.Energy !== undefined) return colors.energy;
        if (cell.Crystal !== undefined) return colors.crystal;
        if (cell.Metal !== undefined) return colors.metal;
        if (cell.Meat !== undefined) return colors.meat;
    }
    return colors.empty;
}

function render(state) {
    if (simWidth !== state.width || simHeight !== state.height) {
        simWidth = state.width;
        simHeight = state.height;
        canvas.width = simWidth * CELL_SIZE;
        canvas.height = simHeight * CELL_SIZE;
    }

    ctx.fillStyle = colors.empty;
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    for (let y = 0; y < state.height; y++) {
        for (let x = 0; x < state.width; x++) {
            const cell = state.map[y][x];
            const color = getCellColor(cell);
            if (color !== colors.empty) {
                ctx.fillStyle = color;
                ctx.fillRect(x * CELL_SIZE, y * CELL_SIZE, CELL_SIZE, CELL_SIZE);
            }
        }
    }

    for (const robot of state.robots) {
        let color = colors.scout;
        if (robot.r_type === 'Collector') color = colors.collector;
        else if (robot.r_type === 'Army') color = colors.army;

        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(robot.x * CELL_SIZE + CELL_SIZE / 2, robot.y * CELL_SIZE + CELL_SIZE / 2, CELL_SIZE / 2.5, 0, Math.PI * 2);
        ctx.fill();
    }

    ctx.fillStyle = colors.enemy;
    for (const enemy of state.enemies) {
        ctx.fillRect(enemy.x * CELL_SIZE + 1, enemy.y * CELL_SIZE + 1, CELL_SIZE - 2, CELL_SIZE - 2);
    }

    ctx.fillStyle = colors.meteorite;
    for (const anim of state.meteorite_anims) {
        ctx.globalAlpha = Math.max(0.1, 1 - (anim.frame / 8));
        ctx.fillRect(anim.x * CELL_SIZE, anim.y * CELL_SIZE, CELL_SIZE, CELL_SIZE);
        ctx.globalAlpha = 1.0;
    }

    ctx.fillStyle = '#fde047';
    for (const flight of state.meteorite_flights) {
        ctx.beginPath();
        ctx.arc(flight.x * CELL_SIZE + CELL_SIZE / 2, flight.y * CELL_SIZE + CELL_SIZE / 2, CELL_SIZE / 3, 0, Math.PI * 2);
        ctx.fill();
    }
}

connect();
