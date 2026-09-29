"""Dev loop: workflow + TableStep(schema) + DataStep(rows, cols, y, operator 'dev') + CubeQueryTask + XYAxis.
   STUDIO_TOK=<tokenfile> python dev/devstep.py <projectId> <schemaId> <rows> <cols> <y> [prop=value ...]
   Prints WF and STEP for `WORKFLOW_ID=.. STEP_ID=.. target/release/dev`."""
import os, sys, uuid, json
import tercen.model.impl as m
from tercen.client.factory import TercenClient
tok = open(os.environ["STUDIO_TOK"]).read().strip()
c = TercenClient("http://127.0.0.1:5402"); c.userService.tercenClient.token = tok; c.httpClient.authorization = tok
proj_id, schema_id, rows, cols, y = sys.argv[1:6]; props = dict(a.split("=", 1) for a in sys.argv[6:])
proj = c.projectService.get(proj_id); sch = c.tableSchemaService.get(schema_id); types = {col.name: col.type for col in sch.columns}
def rect(x, y, w=200.0, h=55.0):
    r = m.Rectangle(); r.topLeft = m.Point(); r.topLeft.x = x; r.topLeft.y = y; r.extent = m.Point(); r.extent.x = w; r.extent.y = h; return r
def factor(n, t=None):
    f = m.Factor(); f.name = n; f.type = t or types.get(n, "string"); return f
def gf(n, t=None):
    g = m.GraphicalFactor(); g.factor = factor(n, t); g.rectangle = rect(0.0, 0.0, 60.0, 30.0); return g
def ctable(fs):
    t = m.CrosstabTable(); t.cellSize = 250.0; t.offset = 0; t.nRows = 0; t.graphicalFactors = fs; t.rectangleSelections = []; return t
def port(cls, name):
    p = cls(); p.id = str(uuid.uuid4()); p.name = name; p.linkType = "relation"; return p
wf = m.Workflow(); wf.name = "fraction_above dev"; wf.projectId = proj.id; wf.acl = m.Acl(); wf.acl.owner = proj.acl.owner; wf.acl.aces = []
wf.folderId = ""; wf.isHidden = False; wf.isPublic = False; wf.isDeleted = False; wf.steps = []; wf.links = []
wf = c.workflowService.create(wf)
ts = m.TableStep(); ts.id = str(uuid.uuid4()); ts.name = "input"; ts.groupId = ""; ts.description = ""; ts.inputs = []; ts.outputs = [port(m.OutputPort, "table")]
ts.rectangle = rect(100.0, 100.0); ts.state = m.StepState(); ts.state.taskId = ""; ts.state.taskState = m.DoneState()
rel = m.SimpleRelation(); rel.id = schema_id; rel.index = 0; ts.model = m.TableStepModel(); ts.model.relation = rel; ts.model.filterSelector = ""
ds = m.DataStep(); ds.id = str(uuid.uuid4()); ds.name = "fraction above (dev)"; ds.groupId = ""; ds.description = ""; ds.parentDataStepId = ""
ip = port(m.InputPort, "data"); ds.inputs = [ip]; ds.outputs = [port(m.OutputPort, "data")]; ds.rectangle = rect(100.0, 250.0)
ds.state = m.StepState(); ds.state.taskId = ""; ds.state.taskState = m.InitState()
ct = m.Crosstab(); ct.taskId = ""; ct.axis = m.XYAxisList(); ct.axis.rectangleSelections = []; ct.axis.xyAxis = []
ct.columnTable = ctable([gf(f) for f in cols.split(",") if f]); ct.rowTable = ctable([gf(f) for f in rows.split(",") if f])
ct.filters = m.Filters(); ct.filters.removeNaN = False; ct.filters.namedFilters = []
st = m.OperatorSettings(); st.namespace = "ds0"; st.environment = []
ref = m.OperatorRef(); ref.name = "dev"; ref.version = "dev"; ref.operatorId = ""; ref.operatorKind = ""; ref.url = m.Url(); ref.url.uri = ""
ref.propertyValues = []
for k, v in props.items():
    pv = m.PropertyValue(); pv.name = k; pv.value = v; ref.propertyValues.append(pv)
ref.operatorSpec = m.OperatorSpec(); ref.operatorSpec.inputSpecs = []; ref.operatorSpec.outputSpecs = []
st.operatorRef = ref; ct.operatorSettings = st; ds.model = ct
link = m.Link(); link.id = str(uuid.uuid4()); link.inputId = ip.id; link.outputId = ts.outputs[0].id
wf.steps = [ts, ds]; wf.links = [link]; c.workflowService.update(wf)
# CubeQueryTask with the y axis
q = m.CubeQuery(); q.relation = rel; q.colColumns = [factor(f) for f in cols.split(",") if f]; q.rowColumns = [factor(f) for f in rows.split(",") if f]
aq = m.CubeAxisQuery(); aq.chartType = "point"; aq.pointSize = 4
aq.xAxis = factor("", "string"); aq.yAxis = factor(y, "double"); aq.colors = []; aq.errors = []; aq.labels = []; aq.preprocessors = []
aq.xAxisSettings = m.AxisSettings(); aq.xAxisSettings.meta = []; aq.yAxisSettings = m.AxisSettings(); aq.yAxisSettings.meta = []
q.axisQueries = [aq]; q.filters = ct.filters; q.operatorSettings = st
task = m.CubeQueryTask(); task.state = m.InitState(); task.owner = wf.acl.owner; task.projectId = wf.projectId; task.query = q
task = c.taskService.create(task); c.taskService.runTask(task.id); task = c.taskService.waitDone(task.id)
print("CUBE", type(task.state).__name__, getattr(task.state, "reason", ""))
wf = c.workflowService.get(wf.id); ds = next(s for s in wf.steps if s.id == ds.id); ds.model.taskId = task.id
axis = json.loads(open(os.path.join(os.path.dirname(__file__), "axis_template.json")).read()) if os.path.exists(os.path.join(os.path.dirname(__file__), "axis_template.json")) else None
if axis is None:
    axis = json.loads('{"kind":"XYAxisList","xyAxis":[{"kind":"XYAxis","chart":{"kind":"ChartPoint","name":"","pointSize":4,"properties":{"kind":"Properties","properties":[],"propertyValues":[]}},"xAxis":{"kind":"Axis","axisExtent":{"x":80.0,"y":30.0,"kind":"Point"},"axisSettings":{"kind":"AxisSettings","meta":[]},"graphicalFactor":{"kind":"GraphicalFactor","factor":{"kind":"Factor","name":"","type":"string"},"rectangle":{"kind":"Rectangle","extent":{"x":0.0,"y":0.0,"kind":"Point"},"topLeft":{"x":0.0,"y":0.0,"kind":"Point"}}}},"yAxis":{"kind":"Axis","axisExtent":{"x":80.0,"y":30.0,"kind":"Point"},"axisSettings":{"kind":"AxisSettings","meta":[]},"graphicalFactor":{"kind":"GraphicalFactor","factor":{"kind":"Factor","name":"value","type":"double"},"rectangle":{"kind":"Rectangle","extent":{"x":0.0,"y":0.0,"kind":"Point"},"topLeft":{"x":0.0,"y":0.0,"kind":"Point"}}}},"colors":{"kind":"Colors","factors":[],"palette":{"kind":"CategoryPalette","backcolor":0,"colorList":{"kind":"ColorList","name":""},"properties":[],"stringColorElements":[]}},"errors":{"kind":"Errors","factors":[]},"labels":{"kind":"Labels","factors":[]},"taskId":"","preprocessors":[]}],"rectangleSelections":[]}')
axis["xyAxis"][0]["yAxis"]["graphicalFactor"]["factor"]["name"] = y; axis["xyAxis"][0]["taskId"] = task.id
ds.model.axis = m.XYAxisList(axis); c.workflowService.update(wf)
print(f"WF={wf.id} STEP={ds.id}")
