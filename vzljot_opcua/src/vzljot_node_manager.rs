use std::{
    sync::{Arc, }, 
    time::Duration,
    collections::HashMap,
};
use async_trait::async_trait;

use opcua_server::{
    address_space::{AddressSpace, read_node_value, write_node_value}, 
    node_manager::{
        HistoryNode, HistoryResult, HistoryUpdateNode, NodeManagerBuilder, NodeManagerCollection, NodeManagersRef, RequestContext, 
        ServerContext, SyncSampler, ParsedReadValueId, 
        memory::{
            InMemoryNodeManager, InMemoryNodeManagerBuilder, InMemoryNodeManagerImpl,
            InMemoryNodeManagerImplBuilder, 
        },
    },
};

use opcua::server::diagnostics::NamespaceMetadata;

use opcua_nodes::{HasNodeId, NodeSetImport};
use opcua_core::{trace_read_lock, trace_write_lock};
use opcua_core::sync::RwLock;
use opcua_types::{
    AttributeId, DataTypeId::HistoryData, DataValue, MonitoringMode, NodeClass, NodeId, NumericRange, ReadAnnotationDataDetails, 
    ReadAtTimeDetails, ReadEventDetails, ReadProcessedDetails, ReadRawModifiedDetails, StatusCode, TimestampsToReturn, Variant, 
};

use crate::Device;

// Node manager impl for the vzljot namespace.
pub struct VzljotNodeManagerImpl{
    write_cbs: RwLock<HashMap<NodeId, WriteCB>>,
    read_cbs: RwLock<HashMap<NodeId, ReadCB>>,
    method_cbs: RwLock<HashMap<NodeId, MethodCB>>,
    namespaces: Vec<NamespaceMetadata>,
    #[allow(unused)]
    node_managers: NodeManagersRef,
    name: String,
    samplers: SyncSampler,
    device: Device,
}

/// Node manager for the vzljot namespace.
pub type VzljotNodeManager = InMemoryNodeManager<VzljotNodeManagerImpl>;

type WriteCB = Arc<dyn Fn(DataValue, &NumericRange) -> StatusCode + Send + Sync + 'static>;
type ReadCB = Arc<
    dyn Fn(&NumericRange, TimestampsToReturn, f64) -> Result<DataValue, StatusCode>
        + Send
        + Sync
        + 'static,
>;
type MethodCB = Arc<dyn Fn(&[Variant]) -> Result<Vec<Variant>, StatusCode> + Send + Sync + 'static>;

/// Builder for the [VzljotNodeManager].
pub struct VzljotNodeManagerBuilder{
    namespaces: Vec<NamespaceMetadata>,
    name: String,
    imports: Vec<Box<dyn NodeSetImport>>,
    device: Device,
}

impl VzljotNodeManagerBuilder{
    /// Create a new Vzljot node manager builder with the given namespace
    /// and name.
    pub fn new(namespace: NamespaceMetadata, name: &str, device: Device) -> Self {
        Self {
            namespaces: vec![namespace],
            name: name.to_owned(),
            imports: Vec::new(),
            device: device,
        }
    }
    /// Create a new simple node manager that imports from the given list
    /// of [NodeSetImport]s.
    pub fn new_imports(imports: Vec<Box<dyn NodeSetImport>>, name: &str, device: Device) -> Self {
        Self {
            namespaces: Vec::new(),
            imports,
            name: name.to_owned(),
            device: device,
        }
    }
}

impl InMemoryNodeManagerImplBuilder for VzljotNodeManagerBuilder {
    type Impl = VzljotNodeManagerImpl;

    fn build(mut self, context: ServerContext, address_space: &mut AddressSpace) -> Self::Impl {     
        {
            let mut type_tree = context.type_tree.write();
            for import in self.imports {
                address_space.import_node_set(&*import, type_tree.namespaces_mut());
                let nss = import.get_own_namespaces();
                for ns in nss {
                    if !self.namespaces.iter().any(|n| n.namespace_uri == ns) {
                        self.namespaces.push(NamespaceMetadata {
                            namespace_uri: ns,
                            ..Default::default()
                        });
                    }
                }
            }
            for ns in &mut self.namespaces {
                ns.namespace_index = type_tree.namespaces_mut().add_namespace(&ns.namespace_uri);
            }
        } 
        for ns in &self.namespaces {
            address_space.add_namespace(&ns.namespace_uri, ns.namespace_index);
        }          
        VzljotNodeManagerImpl::new(self.namespaces, &self.name, context.node_managers.clone(), self.device)
    }
}

pub fn vzljot_node_manager(namespace: NamespaceMetadata, name: &str, device: Device) -> impl NodeManagerBuilder +use<>{
    InMemoryNodeManagerBuilder::new(VzljotNodeManagerBuilder::new(namespace, name, device))
}

#[async_trait]
impl InMemoryNodeManagerImpl for VzljotNodeManagerImpl {
    async fn init(&self, _address_space: &mut AddressSpace, context: ServerContext) {
        self.samplers.run(
            Duration::from_millis(
                context
                    .info
                    .config
                    .limits
                    .subscriptions
                    .min_sampling_interval_ms as u64,
            ),
            context.subscriptions.clone(),
        );
    }
    
   fn namespaces(&self) -> Vec<NamespaceMetadata> {
        self.namespaces.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn read_values(
        &self,
        context: &RequestContext,
        address_space: &RwLock<AddressSpace>,
        nodes: &[&ParsedReadValueId],
        max_age: f64,
        timestamps_to_return: TimestampsToReturn,
    ) -> Vec<DataValue> {
        let address_space = address_space.read();
        let cbs = trace_read_lock!(self.read_cbs);

        nodes
            .iter()
            .map(|n| {
                self.read_node_value(
                    &cbs,
                    context,
                    &address_space,
                    n,
                    max_age,
                    timestamps_to_return,
                )
            })
            .collect()
    }

    /// Perform the history read raw modified service. This should write results
    /// to the `nodes` list of type either `HistoryData` or `HistoryModifiedData`
    ///
    /// Nodes are verified to be readable before this is called.
    async fn history_read_raw_modified(
        &self,
        _context: &RequestContext,
        details: &ReadRawModifiedDetails,
        nodes: &mut [&mut &mut HistoryNode],
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<(), StatusCode> {
        println!("{:?} {:?}", details.start_time, details.end_time);
        if details.is_read_modified == false {        
            for node in nodes{
                let (hdv, status_node) = 
                    crate::request_period(&self.device, details.start_time, details.end_time, 
                        timestamps_to_return, details.return_bounds, details.num_values_per_node);
                node.set_result(opcua_types::HistoryData {data_values: hdv});
                node.set_status(status_node);
            }
            return Ok(())
        }
        Err(StatusCode::BadHistoryOperationUnsupported)
    }

    /// Perform the history read processed service. This should write results
    /// to the `nodes` list of type `HistoryData`.
    ///
    /// Nodes are verified to be readable before this is called.
    async fn history_read_processed(
        &self,
        context: &RequestContext,
        details: &ReadProcessedDetails,
        nodes: &mut [&mut &mut HistoryNode],
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<(), StatusCode> {
        Err(StatusCode::BadHistoryOperationUnsupported)
    }

    /// Perform the history read processed service. This should write results
    /// to the `nodes` list of type `HistoryData`.
    ///
    /// Nodes are verified to be readable before this is called.
    async fn history_read_at_time(
        &self,
        context: &RequestContext,
        details: &ReadAtTimeDetails,
        nodes: &mut [&mut &mut HistoryNode],
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<(), StatusCode> {
        Err(StatusCode::BadHistoryOperationUnsupported)
    }

    /// Perform the history read events service. This should write results
    /// to the `nodes` list of type `HistoryEvent`.
    ///
    /// Nodes are verified to be readable before this is called.
    async fn history_read_events(
        &self,
        context: &RequestContext,
        details: &ReadEventDetails,
        nodes: &mut [&mut &mut HistoryNode],
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<(), StatusCode> {
        Err(StatusCode::BadHistoryOperationUnsupported)
    }

    /// Perform the history read annotations data service. This should write
    /// results to the `nodes` list of type `Annotation`.
    ///
    /// Nodes are verified to be readable before this is called.
    async fn history_read_annotations(
        &self,
        context: &RequestContext,
        details: &ReadAnnotationDataDetails,
        nodes: &mut [&mut &mut HistoryNode],
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<(), StatusCode> {
        Err(StatusCode::BadHistoryOperationUnsupported)
    }

    /// Perform the HistoryUpdate service. This should write result
    /// status codes to the `nodes` list as appropriate.
    ///
    /// Nodes are verified to be writable before this is called.
    async fn history_update(
        &self,
        context: &RequestContext,
        nodes: &mut [&mut &mut HistoryUpdateNode],
    ) -> Result<(), StatusCode> {
        Err(StatusCode::BadHistoryOperationUnsupported)
    }

}

impl VzljotNodeManagerImpl{
    fn new(namespaces: Vec<NamespaceMetadata>, name: &str, node_managers: NodeManagersRef, device: Device) -> Self {
        Self {
            write_cbs: Default::default(),
            read_cbs: Default::default(),
            method_cbs: Default::default(),
            namespaces,
            name: name.to_owned(),
            node_managers,
            samplers: SyncSampler::new(),
            device: device,
        }
    }

    fn read_node_value(
        &self,
        cbs: &HashMap<NodeId, ReadCB>,
        context: &RequestContext,
        address_space: &AddressSpace,
        node_to_read: &ParsedReadValueId,
        max_age: f64,
        timestamps_to_return: TimestampsToReturn,
    ) -> DataValue {
        let mut result_value = DataValue::null();
        // Check that the read is permitted.
        let node = match address_space.validate_node_read(context, node_to_read) {
            Ok(n) => n,
            Err(e) => {
                result_value.status = Some(e);
                return result_value;
            }
        };

        // If there is a callback registered, call that, otherwise read it from the node hierarchy.
        if let Some(cb) = cbs.get(&node_to_read.node_id) {
            match cb(&node_to_read.index_range, timestamps_to_return, max_age) {
                Err(e) => DataValue {
                    status: Some(e),
                    ..Default::default()
                },
                Ok(v) => v,
            }
        } else {
            // If it can't be found, read it from the node hierarchy.
            read_node_value(node, context, node_to_read, max_age, timestamps_to_return)
        }
    }

    pub fn add_read_callback(
        &self,
        id: NodeId,
        cb: impl Fn(&NumericRange, TimestampsToReturn, f64) -> Result<DataValue, StatusCode>
            + Send
            + Sync
            + 'static,
    ) {
        let mut cbs = trace_write_lock!(self.read_cbs);
        cbs.insert(id, Arc::new(cb));
    }

    pub fn get_device(&self) -> Device {
        self.device.clone()
    }
}