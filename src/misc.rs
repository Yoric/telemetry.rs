pub struct NamedStorage<T: ?Sized> {
    pub name: String,
    pub contents: Box<T>,
}


pub enum SerializationFormat {
    Simple,
}

