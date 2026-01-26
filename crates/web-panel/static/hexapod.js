import * as THREE from 'https://esm.sh/three@0.160.0';
import { OrbitControls } from 'https://esm.sh/three@0.160.0/examples/jsm/controls/OrbitControls.js';

const container = document.getElementById('hexapod-visualizer');
if (!container) {
	console.warn('Hexapod visualizer container not found');
} else {
	const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
	renderer.setPixelRatio(window.devicePixelRatio || 1);
	renderer.setSize(container.clientWidth, container.clientHeight);
	renderer.setClearColor(0x000000, 0);
	container.appendChild(renderer.domElement);

	const scene = new THREE.Scene();

	const camera = new THREE.PerspectiveCamera(
		45,
		container.clientWidth / container.clientHeight,
		0.1,
		2000
	);
	camera.position.set(240, 180, 240);

	const controls = new OrbitControls(camera, renderer.domElement);
	controls.enableDamping = true;
	controls.dampingFactor = 0.08;
	controls.target.set(0, 40, 0);
	controls.update();

	const ambient = new THREE.AmbientLight(0xffffff, 0.6);
	scene.add(ambient);
	const keyLight = new THREE.DirectionalLight(0xffffff, 0.8);
	keyLight.position.set(200, 300, 100);
	scene.add(keyLight);
	const rimLight = new THREE.DirectionalLight(0x88aaff, 0.4);
	rimLight.position.set(-200, 200, -150);
	scene.add(rimLight);

	const grid = new THREE.GridHelper(400, 20, 0x334466, 0x223344);
	grid.position.y = -60;
	scene.add(grid);

	const hexapod = new THREE.Group();
	scene.add(hexapod);

	const bodyMaterial = new THREE.MeshStandardMaterial({
		color: 0x667eea,
		metalness: 0.4,
		roughness: 0.35
	});
	const bodyGeometry = new THREE.BoxGeometry(140, 20, 90);
	const bodyMesh = new THREE.Mesh(bodyGeometry, bodyMaterial);
	bodyMesh.position.y = 40;
	hexapod.add(bodyMesh);

	const eyeMaterial = new THREE.MeshStandardMaterial({
		color: 0x36f5ff,
		emissive: 0x36f5ff,
		emissiveIntensity: 0.8,
		metalness: 0.1,
		roughness: 0.2
	});
	const eyeGeometry = new THREE.SphereGeometry(4, 16, 16);
	const leftEye = new THREE.Mesh(eyeGeometry, eyeMaterial);
	const rightEye = new THREE.Mesh(eyeGeometry, eyeMaterial);
	const eyeOffsetX = 70;
	const eyeOffsetY = 42;
	const eyeOffsetZ = 12;
	leftEye.position.set(eyeOffsetX, eyeOffsetY, -eyeOffsetZ);
	rightEye.position.set(eyeOffsetX, eyeOffsetY, eyeOffsetZ);
	hexapod.add(leftEye, rightEye);

	const legMaterial = new THREE.MeshStandardMaterial({
		color: 0xeeeeee,
		metalness: 0.2,
		roughness: 0.6
	});

	const coxaLength = 28;
	const femurLength = 55;
	const tibiaLength = 70;

	const legConfigs = [
		{ id: 'lf', name: 'left_front', x: 55, z: -40, side: -1, phase: 0.0 },
		{ id: 'lm', name: 'left_middle', x: 0, z: -48, side: -1, phase: 0.5 },
		{ id: 'lb', name: 'left_back', x: -55, z: -40, side: -1, phase: 0.0 },
		{ id: 'rf', name: 'right_front', x: 55, z: 40, side: 1, phase: 0.5 },
		{ id: 'rm', name: 'right_middle', x: 0, z: 48, side: 1, phase: 0.0 },
		{ id: 'rb', name: 'right_back', x: -55, z: 40, side: 1, phase: 0.5 }
	];

	function createLeg(config) {
		const legRoot = new THREE.Group();
		legRoot.position.set(config.x, 40, config.z);

		const coxaPivot = new THREE.Group();
		legRoot.add(coxaPivot);

		const coxaGeom = new THREE.BoxGeometry(10, 10, coxaLength);
		const coxaMesh = new THREE.Mesh(coxaGeom, legMaterial);
		coxaMesh.position.z = (coxaLength / 2) * config.side;
		coxaPivot.add(coxaMesh);

		const femurPivot = new THREE.Group();
		femurPivot.position.z = coxaLength * config.side;
		coxaPivot.add(femurPivot);

		const femurGeom = new THREE.BoxGeometry(10, femurLength, 10);
		const femurMesh = new THREE.Mesh(femurGeom, legMaterial);
		femurMesh.position.y = -femurLength / 2;
		femurPivot.add(femurMesh);

		const tibiaPivot = new THREE.Group();
		tibiaPivot.position.y = -femurLength;
		femurPivot.add(tibiaPivot);

		const tibiaGeom = new THREE.BoxGeometry(8, tibiaLength, 8);
		const tibiaMesh = new THREE.Mesh(tibiaGeom, legMaterial);
		tibiaMesh.position.y = -tibiaLength / 2;
		tibiaPivot.add(tibiaMesh);

		return { config, legRoot, coxaPivot, femurPivot, tibiaPivot };
	}

	const legs = legConfigs.map((cfg) => {
		const leg = createLeg(cfg);
		hexapod.add(leg.legRoot);
		return leg;
	});

	let pose = { roll: 0, pitch: 0, yaw: 0 };
	let movement = { forward: 0, strafe: 0, rotation: 0 };
	let gaitPhase = 0;
	let gaitName = '--';
	let lastStatusAt = 0;
	let lastPhaseAt = 0;
	let lastPhaseValue = 0;
	let speedEstimate = 0;

	const API_BASE = `${window.location.protocol}//${window.location.hostname}:3000/api`;

	const gaitBadge = document.getElementById('viz-gait');
	const speedBadge = document.getElementById('viz-speed');
	const resetBtn = document.getElementById('viz-reset');

	if (resetBtn) {
		resetBtn.addEventListener('click', () => {
			camera.position.set(240, 180, 240);
			controls.target.set(0, 40, 0);
			controls.update();
		});
	}

	window.addEventListener('hexapod:pose', (event) => {
		if (!event.detail) return;
		pose = {
			roll: event.detail.roll || 0,
			pitch: event.detail.pitch || 0,
			yaw: event.detail.yaw || 0
		};
	});

	window.addEventListener('hexapod:move', (event) => {
		if (!event.detail) return;
		movement = {
			forward: event.detail.forward || 0,
			strafe: event.detail.strafe || 0,
			rotation: event.detail.rotation || 0
		};    
	});

	window.addEventListener('hexapod:status', (event) => {
		if (!event.detail) return;
		gaitPhase = typeof event.detail.gait_phase === 'number' ? event.detail.gait_phase : gaitPhase;
		gaitName = event.detail.gait_name || gaitName;
		lastStatusAt = performance.now();
	});

	async function pollStatus() {
		try {
			const res = await fetch(`${API_BASE}/status`);
			if (!res.ok) return;
			const status = await res.json();
			if (typeof status.gait_phase === 'number') {
				const now = performance.now();
				const phase = status.gait_phase;
				if (lastPhaseAt > 0) {
					let delta = phase - lastPhaseValue;
					if (delta < -0.5) delta += 1.0;
					if (delta > 0.5) delta -= 1.0;
					const dt = Math.max(0.001, (now - lastPhaseAt) / 1000);
					speedEstimate = Math.min(200, Math.abs(delta / dt) * 120);
				}
				lastPhaseValue = phase;
				lastPhaseAt = now;
				gaitPhase = phase;
			}
			if (status.gait_name) gaitName = status.gait_name;
			lastStatusAt = performance.now();
		} catch (_) {
			// ignore
		}
	}

	setInterval(pollStatus, 200);

	function updateBadges(speed) {
		if (gaitBadge) gaitBadge.textContent = `Gait: ${gaitName}`;
		if (speedBadge) speedBadge.textContent = `Speed: ${speed.toFixed(0)} mm/s`;
	}

	function animate(time) {
		requestAnimationFrame(animate);

		const commandSpeed = Math.min(120, Math.hypot(movement.forward, movement.strafe));
		const speed = Math.max(commandSpeed, speedEstimate);
		const normalizedSpeed = Math.min(1, speed / 120);

		if (performance.now() - lastStatusAt > 1500) {
			gaitPhase = (gaitPhase + 0.01 + normalizedSpeed * 0.06) % 1.0;
		}

		const rollRad = THREE.MathUtils.degToRad(pose.roll || 0);
		const pitchRad = THREE.MathUtils.degToRad(pose.pitch || 0);
		const yawRad = THREE.MathUtils.degToRad(pose.yaw || 0);

		hexapod.rotation.set(pitchRad, yawRad, rollRad);

		const heading = Math.atan2(movement.strafe, movement.forward || 1e-5);
		const yawInfluence = movement.rotation || 0;

		legs.forEach((leg) => {
			const phase = (gaitPhase + leg.config.phase) % 1.0;
			const swing = Math.sin(phase * Math.PI * 2);
			const lift = Math.max(0, swing);

			const baseFemur = -2.0;
			const baseTibia = 2.0;
			const stride = normalizedSpeed * 0.6;

			const coxaSwing = swing * 0.35 * (0.4 + normalizedSpeed);
			leg.coxaPivot.rotation.y =
				heading * 0.4 + yawInfluence * 0.3 * leg.config.side + coxaSwing * leg.config.side;
			const bendSign = leg.config.side < 0 ? -1 : 1;
			leg.femurPivot.rotation.x = (baseFemur + lift * stride) * bendSign;
			leg.tibiaPivot.rotation.x = (baseTibia - lift * stride * 0.6) * bendSign;
		});

		updateBadges(speed);
		controls.update();
		renderer.render(scene, camera);
	}

	const resizeObserver = new ResizeObserver(() => {
		const { clientWidth, clientHeight } = container;
		if (clientWidth === 0 || clientHeight === 0) return;
		camera.aspect = clientWidth / clientHeight;
		camera.updateProjectionMatrix();
		renderer.setSize(clientWidth, clientHeight);
	});
	resizeObserver.observe(container);

	animate();
}