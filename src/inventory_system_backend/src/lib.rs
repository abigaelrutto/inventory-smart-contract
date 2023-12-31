#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};
use validator::Validate;

// Define type aliases for convenience
type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Product {
    id: u64,
    name: String,
    quantity: u32,
    category: String,
    warehouse: Warehouse,
    added_at: u64,
    re_stocked_at: u64,
}

// Implement the 'Storable' traits

impl Storable for Product {
    // Conversion to bytes
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
    // Conversion from bytes
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Warehouse {
    id: u64,
    name: String,
    address: String,
}

impl Storable for Warehouse {
    // Conversion to bytes
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }
    // Conversion from bytes
    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implement the 'BoundedStorable' traits
impl BoundedStorable for Product {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

impl BoundedStorable for Warehouse {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

// Define thread-local static variables for memory management and storage
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static PRODUCT_STORAGE: RefCell<StableBTreeMap<u64, Product, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2)))
    ));

    static WAREHOUSE_STORAGE: RefCell<StableBTreeMap<u64, Warehouse, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(3)))
    ));
}

// Struct for payload date used in update functions
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default, Validate)]
struct WarehousePayload {
    #[validate(length(min = 3))]
    name: String,
    #[validate(length(min = 3))]
    address: String,
    password: String,
    city: String,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default, Validate)]
struct ProductPayload {
    #[validate(length(min = 3))]
    name: String,
    category: String,
    quantity: u32,
    warehouse_id: u64,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct EditProductPayload {
    name: String,
    password: String,
    product_id: u64,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct GetProductPayload {
    product_id: u64,
    amount: u32,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct EditWarehousePayload {
    warehouse_id: u64,
    name: String,
}

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct AccessPayload {
    doctor_id: u64,
    product_id: u64,
    doctor_password: String,
}

// Query function to get all warehouses
#[ic_cdk::query]
fn get_all_warehouses() -> Result<Vec<Warehouse>, Error> {
    // Retrieve all Warehouses from the storage
    let warehousemap: Vec<(u64, Warehouse)> = WAREHOUSE_STORAGE.with(|s| s.borrow().iter().collect());
    // Extract the Warehouses from the tuple and create a vector
    let warehouses: Vec<Warehouse> = warehousemap
        .into_iter()
        .map(|(_, warehouse)| warehouse)
        .collect();

    match warehouses.len() {
        0 => Err(Error::NotFound {
            msg: format!("no Warehouses found"),
        }),
        _ => Ok(warehouses),
    }
}

// Get Warehouses by city and name content
#[ic_cdk::query]
fn get_warehouse_by_name(search: String) -> Result<Vec<Warehouse>, Error> {
    let query = search.to_lowercase();
    // Retrieve all Warehouses from the storage
    let warehouse_map: Vec<(u64, Warehouse)> = WAREHOUSE_STORAGE.with(|s| s.borrow().iter().collect());
    let warehouses: Vec<Warehouse> = warehouse_map
        .into_iter()
        .map(|(_, warehouse)| warehouse)
        .collect();

    // Filter the warehouses by name
    let incomplete_products: Vec<Warehouse> = warehouses
        .into_iter()
        .filter(|warehouse| (warehouse.name).to_lowercase().contains(&query))
        .collect();

    // Check if any warehouses are found
    match incomplete_products.len() {
        0 => Err(Error::NotFound {
            msg: format!("No warehouses for name: {} could be found", query),
        }),
        _ => Ok(incomplete_products),
    }
}

// get warehouse by ID
#[ic_cdk::query]
fn get_warehouse_by_id(id: u64) -> Result<Warehouse, Error> {
    match WAREHOUSE_STORAGE.with(|warehouses| warehouses.borrow().get(&id)) {
        Some(warehouse) => Ok(warehouse),
        None => Err(Error::NotFound {
            msg: format!("warehouse of id: {} not found", id),
        }),
    }
}

// Create new Warehouse
#[ic_cdk::update]
fn add_warehouse(payload: WarehousePayload) -> Result<Warehouse, Error> {
    // validate payload
    let validate_payload = payload.validate();
    if validate_payload.is_err() {
        return Err(Error::InvalidPayload {
            msg: validate_payload.unwrap_err().to_string(),
        });
    }

    let id = ID_COUNTER
        .with(|counter| {
            let current_id = *counter.borrow().get();
            counter.borrow_mut().set(current_id + 1)
        })
        .expect("Cannot increment Ids");

    let warehouse = Warehouse {
        id,
        name: payload.name.clone(),
        address: payload.address,
    };

    match WAREHOUSE_STORAGE.with(|s| s.borrow_mut().insert(id, warehouse.clone())) {
        Some(_) => Err(Error::InvalidPayload {
            msg: format!("Could not add warehouse name: {}", payload.name),
        }),
        None => Ok(warehouse),
    }
}

// update function to edit a warehouse where only owners of warehouses can edit title, is_community, price and description. Non owners can only edit descriptions of communtiy warehouses. authorizations is by password
#[ic_cdk::update]
fn edit_warehouse(payload: EditWarehousePayload) -> Result<Warehouse, Error> {
    let warehouse = WAREHOUSE_STORAGE.with(|warehouses| warehouses.borrow().get(&payload.warehouse_id));

    match warehouse {
        Some(warehouse) => {
            let new_warehouse = Warehouse {
                name: payload.name,
                ..warehouse.clone()
            };

            match WAREHOUSE_STORAGE
                .with(|s| s.borrow_mut().insert(warehouse    .id, new_warehouse    .clone()))
            {
                Some(_) => Ok(new_warehouse),
                None => Err(Error::InvalidPayload {
                    msg: format!("Could not edit warehouse     title: {}", warehouse    .name),
                }),
            }
        }
        None => Err(Error::NotFound {
            msg: format!("warehouse of id: {} not found", payload.warehouse_id),
        }),
    }
}

// Define query function to get a product by ID
#[ic_cdk::query]
fn get_product(id: u64) -> Result<Product, Error> {
    match PRODUCT_STORAGE.with(|products| products.borrow().get(&id)) {
        Some(product) => Ok(product),
        None => Err(Error::NotFound {
            msg: format!("product id:{} does not exist", id),
        }),
    }
}

// Update function to add a product
#[ic_cdk::update]
fn add_product(payload: ProductPayload) -> Result<Product, Error> {
    // validate payload
    let validate_payload = payload.validate();
    if validate_payload.is_err() {
        return Err(Error::InvalidPayload {
            msg: validate_payload.unwrap_err().to_string(),
        });
    }

    let id = ID_COUNTER
        .with(|counter| {
            let current_id = *counter.borrow().get();
            counter.borrow_mut().set(current_id + 1)
        })
        .expect("Cannot increment Ids");

    // get warehouse
    let warehouse = WAREHOUSE_STORAGE.with(|warehouses| warehouses.borrow().get(&payload.warehouse_id));
    match warehouse {
        Some(warehouse) => {
            
            let product = Product {
                id,
                name: payload.name.clone(),
                quantity: payload.quantity,
                category: payload.category,
                warehouse: warehouse.clone(),
                added_at: time(),
                re_stocked_at: time(),
            };

            match PRODUCT_STORAGE.with(|s| s.borrow_mut().insert(id, product.clone())) {
                None => Ok(product),
                Some(_) => Err(Error::InvalidPayload {
                    msg: format!("Could not add product name: {}", payload.name),
                }),
            }
        }
        None => Err(Error::NotFound {
            msg: format!("Warehouse of id: {} not found", payload.warehouse_id),
        }),
    }
}

// function to remove a given quantity fo product from a warehouse while cheking if product is available and if warehouse has enough quantity
#[ic_cdk::update]
fn remove_product_from_warehouse(payload: GetProductPayload) -> Result<Product, Error> {
    let product = PRODUCT_STORAGE.with(|products| products.borrow().get(&payload.product_id));
    match product {
        Some(product) => {
            if product.quantity < payload.amount {
                return Err(Error::InvalidPayload {
                    msg: format!("Not enough quantity of product: {}", product.name),
                });
            }

            let new_product = Product {
                quantity: product.quantity - payload.amount,
                ..product.clone()
            };

            match PRODUCT_STORAGE.with(|s| s.borrow_mut().insert(product.id, new_product.clone())) {
                Some(_) => Ok(new_product),
                None => Err(Error::InvalidPayload {
                    msg: format!("Could not remove product name: {}", product.name),
                }),
            }
        }
        None => Err(Error::NotFound {
            msg: format!("product of id: {} not found", payload.product_id),
        }),
    }
}

// update function to edit a product where authorizations is by password
#[ic_cdk::update]
fn edit_product(payload: EditProductPayload) -> Result<Product, Error> {
    let product = PRODUCT_STORAGE.with(|products| products.borrow().get(&payload.product_id));

    match product {
        Some(product) => {

            let new_product = Product {
                name: payload.name,
                ..product.clone()
            };

            match PRODUCT_STORAGE.with(|s| s.borrow_mut().insert(product.id, new_product.clone())) {
                Some(_) => Ok(new_product),
                None => Err(Error::InvalidPayload {
                    msg: format!("Could not edit product name: {}", product.name),
                }),
            }
        }
        None => Err(Error::NotFound {
            msg: format!("product of id: {} not found", payload.product_id),
        }),
    }
}

// Define an Error enum for handling errors
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    AlreadyInit { msg: String },
    InvalidPayload { msg: String },
    Unauthorized { msg: String },
}

// Candid generator for exporting the Candid interface
ic_cdk::export_candid!();
