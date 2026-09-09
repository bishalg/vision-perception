import "./style.css";
import * as Cesium from "cesium";
import "cesium/Build/Cesium/Widgets/widgets.css";

const statusEl = document.getElementById("status");
const panel = document.getElementById("panel");
const panelTitle = document.getElementById("panel-title");
const panelMeta = document.getElementById("panel-meta");
const panelImage = document.getElementById("panel-image");
const panelPose = document.getElementById("panel-pose");
const panelClose = document.getElementById("panel-close");

panelClose.addEventListener("click", () => panel.classList.add("hidden"));

function headingPitchToOrientation(position, headingDeg, pitchDeg) {
  const heading = Cesium.Math.toRadians(headingDeg);
  const pitch = Cesium.Math.toRadians(pitchDeg);
  const hpr = new Cesium.HeadingPitchRoll(heading, pitch, 0.0);
  return Cesium.Transforms.headingPitchRollQuaternion(position, hpr);
}

function addFrustum(viewer, feature) {
  const [lon, lat, elev] = feature.geometry.coordinates;
  const props = feature.properties;
  const height = Math.max(Number(elev) || 0, 5);
  const position = Cesium.Cartesian3.fromDegrees(lon, lat, height);
  const orientation = headingPitchToOrientation(position, props.heading_deg ?? 0, props.pitch_deg ?? -15);
  const fov = Cesium.Math.toRadians(props.fov_deg ?? 60);
  const aspectRatio = 1.333;
  const near = 2.0;
  const far = 180.0;

  viewer.entities.add({
    id: props.id,
    name: props.name,
    position,
    orientation,
    point: {
      pixelSize: 10,
      color: Cesium.Color.fromCssColorString("#5ec8ff"),
      outlineColor: Cesium.Color.WHITE,
      outlineWidth: 1,
      disableDepthTestDistance: Number.POSITIVE_INFINITY,
    },
    label: {
      text: props.name,
      font: "12px sans-serif",
      fillColor: Cesium.Color.WHITE,
      outlineColor: Cesium.Color.BLACK,
      outlineWidth: 3,
      style: Cesium.LabelStyle.FILL_AND_OUTLINE,
      pixelOffset: new Cesium.Cartesian2(0, -18),
      disableDepthTestDistance: Number.POSITIVE_INFINITY,
      showBackground: true,
      backgroundColor: Cesium.Color.fromCssColorString("#0c182a").withAlpha(0.75),
    },
    // Approximate coverage cone via a translucent ellipsoid volume along look direction.
    ellipsoid: {
      radii: new Cesium.Cartesian3(far * 0.35, far * 0.35, far * 0.55),
      material: Cesium.Color.fromCssColorString("#3aa0ff").withAlpha(0.12),
      outline: true,
      outlineColor: Cesium.Color.fromCssColorString("#7ec8ff").withAlpha(0.55),
    },
    properties: {
      ...props,
      _fov: fov,
      _aspectRatio: aspectRatio,
      _near: near,
      _far: far,
    },
  });
}

function showPanel(props) {
  panelTitle.textContent = props.name || props.id;
  panelMeta.textContent = `${props.agency || "Unknown agency"} · ${props.id}`;
  panelPose.innerHTML = `
    <dt>Lat / Lon</dt><dd>${Number(props.latitude ?? props.lat ?? "").toString() || "—"}</dd>
    <dt>Heading</dt><dd>${props.heading_deg}°</dd>
    <dt>Pitch</dt><dd>${props.pitch_deg}°</dd>
    <dt>FOV</dt><dd>${props.fov_deg}°</dd>
    <dt>Elevation</dt><dd>${props.elevation_m} m</dd>
  `;
  if (props.image) {
    panelImage.src = props.image;
    panelImage.style.display = "block";
  } else {
    panelImage.removeAttribute("src");
    panelImage.style.display = "none";
  }
  panel.classList.remove("hidden");
}

async function main() {
  // OSM + ellipsoid terrain: no Cesium ion token required for demos.
  const viewer = new Cesium.Viewer("cesiumContainer", {
    animation: false,
    timeline: false,
    baseLayerPicker: false,
    geocoder: false,
    homeButton: true,
    sceneModePicker: false,
    navigationHelpButton: false,
    fullscreenButton: true,
    imageryProvider: false,
    terrainProvider: new Cesium.EllipsoidTerrainProvider(),
  });

  viewer.imageryLayers.removeAll();
  viewer.imageryLayers.addImageryProvider(
    new Cesium.UrlTemplateImageryProvider({
      url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
      credit: "© OpenStreetMap contributors",
    }),
  );

  viewer.scene.globe.enableLighting = false;
  viewer.scene.backgroundColor = Cesium.Color.fromCssColorString("#0b1220");

  const response = await fetch("./cameras.geojson");
  if (!response.ok) {
    statusEl.textContent = "Failed to load cameras.geojson — run tools/geojson_exporter.py";
    return;
  }
  const collection = await response.json();
  const features = collection.features || [];
  for (const feature of features) {
    // Attach lat for panel convenience
    feature.properties.lat = feature.geometry.coordinates[1];
    feature.properties.longitude = feature.geometry.coordinates[0];
    feature.properties.latitude = feature.geometry.coordinates[1];
    addFrustum(viewer, feature);
  }

  statusEl.textContent = `${features.length} cameras loaded · click a marker for still + pose`;

  if (features.length > 0) {
    const [lon0, lat0, elev0] = features[0].geometry.coordinates;
    viewer.camera.flyTo({
      destination: Cesium.Cartesian3.fromDegrees(lon0, lat0, Math.max(elev0, 0) + 2500),
      duration: 1.2,
    });
  }

  const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);
  handler.setInputAction((movement) => {
    const picked = viewer.scene.pick(movement.position);
    if (!Cesium.defined(picked) || !picked.id || !picked.id.properties) {
      return;
    }
    const props = {};
    const bag = picked.id.properties;
    bag.propertyNames.forEach((name) => {
      props[name] = bag[name]?.getValue?.() ?? bag[name];
    });
    showPanel(props);
  }, Cesium.ScreenSpaceEventType.LEFT_CLICK);
}

main().catch((err) => {
  statusEl.textContent = `Viewer error: ${err.message || err}`;
  console.error(err);
});
