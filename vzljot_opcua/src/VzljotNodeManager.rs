use async_trait::async_trait;

use opcua::server::node_manager::memory::{
    InMemoryNodeManager, InMemoryNodeManagerBuilder, InMemoryNodeManagerImpl,
    InMemoryNodeManagerImplBuilder, NamespaceMetadata,
};

// Node manager impl for the vzljot namespace.
pub struct VzljotNodeManagerImpl {
    namespaces: Vec<NamespaceMetadata>,
    #[allow(unused)]
    node_managers: NodeManagersRef,
    name: String,
}

/// Node manager for the vzljot namespace.
pub type VzljotNodeManager = InMemoryNodeManager<VzljotNodeManagerImpl>;

/// Builder for the [VzljotNodeManager.
pub struct VzljotNodeManagerBuilder{
    namespaces: Vec<NamespaceMetadata>,
    name: String,
}

impl VzljotNodeManagerBuilder {
    /// Create a new Vzljot node manager builder with the given namespace
    /// and name.
    pub fn new(namespace: NamespaceMetadata, name: &str) -> Self {
        Self {
            namespaces: vec![namespace],
            name: name.to_owned(),
        }
    }
}

impl InMemoryNodeManagerImplBuilder for VzljotNodeManagerBuilder {
    type Impl = VzljotNodeManagerImpl;

    fn build(mut self, context: ServerContext, address_space: &mut AddressSpace) -> Self::Impl {
        for ns in &self.namespaces {
            address_space.add_namespace(&ns.namespace_uri, ns.namespace_index);
        }
        VzljotNodeManagerImpl::new(self.namespaces, &self.name, context.node_managers.clone())
    }
}

pub fn vzljot_node_manager(namespace: NamespaceMetadata, name: &str) -> impl NodeManagerBuilder {
    InMemoryNodeManagerBuilder::new(VzljotNodeManagerBuilder::new(namespace, name))
}

#[async_trait]
impl InMemoryNodeManagerImpl for VzljotNodeManagerImpl {
    async fn init(&self, _address_space: &mut AddressSpace, context: ServerContext) {

    }
    
   fn namespaces(&self) -> Vec<NamespaceMetadata> {
        self.namespaces.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }
}