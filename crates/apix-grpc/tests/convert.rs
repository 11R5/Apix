//! What a definition can be made from, and that it comes out right.

use apix_grpc::prost_reflect::{DescriptorPool, Kind};
use apix_grpc::{
    Certainty, ConvertInput, compare, compile_definition, convert, convert_bytes, find_method,
    methods,
};
use prost::Message as _;

const HELLO: &str = r#"
syntax = "proto3";
package hello.v1;
import "google/protobuf/timestamp.proto";

service Greeter {
  rpc SayHello(HelloRequest) returns (HelloReply);
  rpc Chat(stream HelloRequest) returns (stream HelloReply);
}

message HelloRequest {
  string name = 3;
  repeated int64 ids = 7;
  map<string, int32> counts = 2;
  optional bool loud = 9;
  Kind kind = 4;
  oneof target {
    string email = 10;
    Inner inner = 11;
  }
  message Inner { sint32 a = 1; }
  google.protobuf.Timestamp at = 12;
}

message HelloReply {
  string greeting = 1 [json_name = "GREETING"];
  fixed64 stamp = 5;
}

enum Kind {
  KIND_UNSPECIFIED = 0;
  KIND_A = 5;
}
"#;

fn input(source: &str) -> ConvertInput<'_> {
    ConvertInput {
        source,
        response: None,
        method: None,
    }
}

fn with_method<'a>(source: &'a str, method: &'a str) -> ConvertInput<'a> {
    ConvertInput {
        source,
        response: None,
        method: Some(method),
    }
}

/// The compiled file of `HELLO`, without what a generator leaves out.
fn hello_file() -> prost_types::FileDescriptorProto {
    let pool = compile_definition(HELLO, &[]).unwrap();
    let file = pool.get_file_by_name("definition.proto").unwrap();
    let mut proto = file.file_descriptor_proto().clone();
    proto.name = Some("hello/v1/hello.proto".into());
    proto
}

fn hello_bytes() -> Vec<u8> {
    hello_file().encode_to_vec()
}

fn assert_is_hello(pool: &DescriptorPool) {
    let method = find_method(pool, "hello.v1.Greeter/SayHello").expect("the method");
    assert_eq!(method.input().full_name(), "hello.v1.HelloRequest");
    let chat = find_method(pool, "hello.v1.Greeter/Chat").expect("the stream");
    assert!(chat.is_client_streaming() && chat.is_server_streaming());
    let input = method.input();
    assert_eq!(input.get_field_by_name("name").unwrap().number(), 3);
    assert_eq!(input.get_field_by_name("ids").unwrap().number(), 7);
    assert!(input.get_field_by_name("counts").unwrap().is_map());
    assert!(matches!(
        input.get_field_by_name("kind").unwrap().kind(),
        Kind::Enum(_)
    ));
    assert_eq!(
        input
            .get_field_by_name("email")
            .unwrap()
            .containing_oneof()
            .unwrap()
            .name(),
        "target"
    );
    assert_eq!(
        method
            .output()
            .get_field_by_name("greeting")
            .unwrap()
            .json_name(),
        "GREETING"
    );
    assert_eq!(
        method.output().get_field_by_name("stamp").unwrap().number(),
        5
    );
}

#[test]
fn protobuf_text_is_taken_as_it_is() {
    let c = convert(&input(HELLO)).unwrap();
    assert_eq!(c.certainty, Certainty::Exact);
    assert!(!c.is_guess());
    assert_eq!(c.definition, HELLO.trim());
    assert_is_hello(&c.pool);
}

#[test]
fn protobuf_without_a_syntax_line_is_proto3() {
    let c = convert(&input("message A { string b = 1; }")).unwrap();
    assert!(c.definition.starts_with("syntax = \"proto3\";"));
    assert!(c.notes.iter().any(|n| n.contains("proto3")));
}

#[test]
fn broken_protobuf_says_why() {
    let error = convert(&input("syntax = \"proto3\";\nmessage A { string = 1; }"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("not valid protobuf"), "{error}");
}

#[test]
fn a_binary_descriptor_set_prints_as_text_that_means_the_same() {
    let set = prost_types::FileDescriptorSet {
        file: vec![hello_file()],
    };
    let c = convert_bytes(&set.encode_to_vec(), None, None).unwrap();
    assert_eq!(c.certainty, Certainty::Exact);
    assert!(
        c.definition.starts_with("syntax = \"proto3\";"),
        "{}",
        c.definition
    );
    assert!(c.definition.contains("oneof target"));
    assert!(c.definition.contains("map<string, int32> counts = 2"));
    assert!(c.definition.contains("optional bool loud = 9"));
    assert_is_hello(&compile_definition(&c.definition, &[]).unwrap());
}

#[test]
fn a_descriptor_set_in_json_is_read() {
    let binary = prost_types::FileDescriptorSet {
        file: vec![hello_file()],
    };
    let printed = convert_bytes(&binary.encode_to_vec(), None, None).unwrap();
    // Protobuf's JSON of the set, the way `buf build -o x.json` writes it.
    let name = "hello/v1/hello.proto";
    let json = format!(
        r#"{{"file":[{{"name":"{name}","package":"p","syntax":"proto3",
        "service":[{{"name":"S","method":[{{"name":"M","inputType":".p.A","outputType":".p.A"}}]}}],
        "messageType":[{{"name":"A","field":[{{"name":"b","number":4,"label":"LABEL_OPTIONAL","type":"TYPE_STRING","jsonName":"b"}}]}}]}}]}}"#
    );
    let c = convert(&input(&json)).unwrap();
    assert_eq!(c.certainty, Certainty::Exact, "{}", printed.definition);
    let method = find_method(&c.pool, "p.S/M").unwrap();
    assert_eq!(method.input().get_field_by_name("b").unwrap().number(), 4);
}

#[test]
fn several_files_stay_a_descriptor_set_that_compiles_back() {
    let dependency = prost_types::FileDescriptorProto {
        name: Some("common.proto".into()),
        package: Some("c".into()),
        syntax: Some("proto3".into()),
        message_type: vec![prost_types::DescriptorProto {
            name: Some("Shared".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let main = prost_types::FileDescriptorProto {
        name: Some("main.proto".into()),
        package: Some("m".into()),
        syntax: Some("proto3".into()),
        dependency: vec!["common.proto".into()],
        message_type: vec![prost_types::DescriptorProto {
            name: Some("M".into()),
            field: vec![prost_types::FieldDescriptorProto {
                name: Some("s".into()),
                number: Some(2),
                r#type: Some(prost_types::field_descriptor_proto::Type::Message as i32),
                type_name: Some(".c.Shared".into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let set = prost_types::FileDescriptorSet {
        file: vec![main.clone(), dependency],
    };
    let c = convert_bytes(&set.encode_to_vec(), None, None).unwrap();
    assert!(c.definition.trim_start().starts_with('{'));
    let pool = compile_definition(&c.definition, &[]).unwrap();
    assert!(pool.get_message_by_name("m.M").is_some());

    let missing = prost_types::FileDescriptorSet { file: vec![main] };
    let error = convert_bytes(&missing.encode_to_vec(), None, None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("common.proto"), "{error}");
}

/// `bytes` written the way a generator writes a string: printable as is,
/// the rest octal or hex.
fn escaped(bytes: &[u8], octal: bool) -> String {
    let mut out = String::new();
    for b in bytes {
        match b {
            b'"' => out += "\\\"",
            b'\\' => out += "\\\\",
            b'\n' => out += "\\n",
            b'\'' if !octal => out += "\\'",
            0x20..=0x7e => out.push(*b as char),
            other if octal => out += &format!("\\{other:03o}"),
            other => out += &format!("\\x{other:02x}"),
        }
    }
    out
}

fn assert_embedded(code: &str) {
    let c = convert(&input(code)).unwrap();
    assert_eq!(c.certainty, Certainty::Exact, "{code}");
    assert!(c.format.contains("generated"));
    assert_is_hello(&c.pool);
    assert_is_hello(&compile_definition(&c.definition, &[]).unwrap());
}

#[test]
fn generated_python_holds_the_description() {
    let bytes = hello_bytes();
    assert_embedded(&format!(
        "import sys\nDESCRIPTOR = _descriptor_pool.Default().AddSerializedFile(b'{}')\n_globals = globals()\n",
        escaped(&bytes, false)
    ));
}

#[test]
fn generated_java_holds_the_description() {
    let bytes = hello_bytes();
    let (a, b) = bytes.split_at(bytes.len() / 2);
    assert_embedded(&format!(
        "public final class Hello {{\n  static {{\n    java.lang.String[] descriptorData = {{\n      \"{}\" +\n      \"{}\"\n    }};\n  }}\n}}\n",
        escaped(a, true),
        escaped(b, true)
    ));
}

#[test]
fn generated_csharp_holds_the_description() {
    use base64::Engine as _;
    let text = base64::engine::general_purpose::STANDARD.encode(hello_bytes());
    let (a, b) = text.split_at(text.len() / 2);
    assert_embedded(&format!(
        "public static partial class HelloReflection {{\n  static HelloReflection() {{\n    byte[] descriptorData = global::System.Convert.FromBase64String(\n        string.Concat(\n          \"{a}\",\n          \"{b}\"));\n  }}\n}}\n"
    ));
}

#[test]
fn generated_go_holds_the_description() {
    let bytes = hello_bytes();
    let list = bytes
        .iter()
        .map(|b| format!("0x{b:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    assert_embedded(&format!(
        "package hello\n\nvar file_hello_proto_rawDesc = []byte{{\n\t{list},\n}}\n"
    ));
    let (a, b) = bytes.split_at(bytes.len() / 2);
    assert_embedded(&format!(
        "package hello\n\nconst file_hello_proto_rawDesc = \"\" +\n\t\"{}\" +\n\t\"{}\"\n",
        escaped(a, false),
        escaped(b, false)
    ));
}

#[test]
fn generated_typescript_from_protobuf_es_holds_the_description() {
    use base64::Engine as _;
    let text = base64::engine::general_purpose::STANDARD_NO_PAD.encode(hello_bytes());
    assert_embedded(&format!(
        "import {{ fileDesc }} from \"@bufbuild/protobuf/codegenv1\";\nexport const file_hello: GenFile = /*@__PURE__*/\n  fileDesc(\"{text}\", [file_google_protobuf_timestamp]);\n"
    ));
}

#[test]
fn go_struct_tags_give_numbers_and_types() {
    let code = r#"
package hello

type Kind int32

const (
	Kind_KIND_UNSPECIFIED Kind = 0
	Kind_KIND_A           Kind = 5
)

type HelloRequest struct {
	state         protoimpl.MessageState
	Name  string           `protobuf:"bytes,3,opt,name=name,proto3" json:"name,omitempty"`
	Ids   []int64          `protobuf:"varint,7,rep,packed,name=ids,proto3" json:"ids,omitempty"`
	Hash  uint64           `protobuf:"fixed64,8,opt,name=hash,proto3" json:"hash,omitempty"`
	Delta int32            `protobuf:"zigzag32,9,opt,name=delta,proto3" json:"delta,omitempty"`
	Blob  []byte           `protobuf:"bytes,10,opt,name=blob,proto3" json:"blob,omitempty"`
	Kind  Kind             `protobuf:"varint,4,opt,name=kind,proto3,enum=hello.Kind" json:"kind,omitempty"`
	Inner *Inner           `protobuf:"bytes,11,opt,name=inner,proto3" json:"inner,omitempty"`
	Counts map[string]int32 `protobuf:"bytes,2,rep,name=counts,proto3" json:"counts,omitempty" protobuf_key:"bytes,1,opt,name=key,proto3" protobuf_val:"varint,2,opt,name=value,proto3"`
}

type Inner struct {
	A float64 `protobuf:"fixed64,1,opt,name=a,proto3" json:"a,omitempty"`
}

type HelloReply struct {
	Greeting string `protobuf:"bytes,1,opt,name=greeting,proto3" json:"greeting,omitempty"`
}

const Greeter_SayHello_FullMethodName = "/hello.Greeter/SayHello"
"#;
    let c = convert(&input(code)).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    assert!(c.is_guess());
    assert_eq!(c.format, "Go");
    let method = find_method(&c.pool, "hello.Greeter/SayHello").expect(&c.definition);
    assert_eq!(method.output().name(), "HelloReply");
    let request = method.input();
    let number = |name: &str| request.get_field_by_name(name).unwrap().number();
    assert_eq!(number("name"), 3);
    assert_eq!(number("counts"), 2);
    assert!(request.get_field_by_name("state").is_none());
    assert!(
        c.definition.contains("repeated int64 ids = 7"),
        "{}",
        c.definition
    );
    assert!(c.definition.contains("fixed64 hash = 8"));
    assert!(c.definition.contains("sint32 delta = 9"));
    assert!(c.definition.contains("bytes blob = 10"));
    assert!(c.definition.contains("Kind kind = 4"));
    assert!(c.definition.contains("map<string, int32> counts = 2"));
    assert!(c.definition.contains("double a = 1"));
    assert!(c.definition.contains("KIND_A = 5"));
}

#[test]
fn prost_attributes_give_numbers_and_types() {
    let code = r#"
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HelloRequest {
    #[prost(string, tag = "3")]
    pub name: ::prost::alloc::string::String,
    #[prost(int64, repeated, tag = "7")]
    pub ids: ::prost::alloc::vec::Vec<i64>,
    #[prost(message, optional, tag = "11")]
    pub inner: ::core::option::Option<Inner>,
    #[prost(enumeration = "Kind", tag = "4")]
    pub kind: i32,
    #[prost(map = "string, int32", tag = "2")]
    pub counts: ::std::collections::HashMap<::prost::alloc::string::String, i32>,
    #[prost(sint32, tag = "9")]
    pub delta: i32,
    #[prost(bytes = "vec", tag = "10")]
    pub blob: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Inner {
    #[prost(fixed32, tag = "1")]
    pub a: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HelloReply {
    #[prost(string, tag = "1")]
    pub greeting: ::prost::alloc::string::String,
}
#[repr(i32)]
pub enum Kind {
    Unspecified = 0,
    A = 5,
}
"#;
    let c = convert(&with_method(code, "hello.Greeter/SayHello")).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    assert_eq!(c.format, "Rust");
    for line in [
        "string name = 3",
        "repeated int64 ids = 7",
        "Inner inner = 11",
        "Kind kind = 4",
        "map<string, int32> counts = 2",
        "sint32 delta = 9",
        "bytes blob = 10",
        "fixed32 a = 1",
        "A = 5",
    ] {
        assert!(c.definition.contains(line), "{line} in {}", c.definition);
    }
}

#[test]
fn ts_proto_encoders_give_numbers_and_types() {
    let code = r#"
export interface HelloRequest {
  name: string;
  ids: number[];
  delta: number;
  inner: Inner | undefined;
}

export interface Inner {
  a: string;
}

export const HelloRequest: MessageFns<HelloRequest> = {
  encode(message: HelloRequest, writer: BinaryWriter = new BinaryWriter()): BinaryWriter {
    if (message.name !== "") {
      writer.uint32(26).string(message.name);
    }
    writer.uint32(58).fork();
    for (const v of message.ids) {
      writer.int64(v);
    }
    writer.join();
    if (message.delta !== 0) {
      writer.uint32(72).sint32(message.delta);
    }
    if (message.inner !== undefined) {
      Inner.encode(message.inner, writer.uint32(90).fork()).join();
    }
    return writer;
  },

  decode(input: BinaryReader | Uint8Array, length?: number): HelloRequest {
    return message;
  },
};

export const Inner: MessageFns<Inner> = {
  encode(message: Inner, writer: BinaryWriter = new BinaryWriter()): BinaryWriter {
    if (message.a !== "") {
      writer.uint32(10).string(message.a);
    }
    return writer;
  },
};
"#;
    let c = convert(&with_method(code, "hello.Greeter/SayHello")).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    for line in [
        "string name = 3",
        "repeated int64 ids = 7",
        "sint32 delta = 9",
        "Inner inner = 11",
        "string a = 1",
    ] {
        assert!(c.definition.contains(line), "{line} in {}", c.definition);
    }
}

#[test]
fn betterproto_and_annotations_give_numbers() {
    let python = r#"
@dataclass
class HelloRequest(betterproto.Message):
    name: str = betterproto.string_field(3)
    ids: List[int] = betterproto.int64_field(7)
    delta: int = betterproto.sint32_field(9)
"#;
    let c = convert(&with_method(python, "h.G/Say")).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    assert!(c.definition.contains("string name = 3"), "{}", c.definition);
    assert!(c.definition.contains("repeated int64 ids = 7"));
    assert!(c.definition.contains("sint32 delta = 9"));

    let csharp = r#"
[ProtoContract]
public class HelloRequest {
    [ProtoMember(3)] public string Name { get; set; }
    [ProtoMember(7)] public List<long> Ids { get; set; }
}
"#;
    let c = convert(&with_method(csharp, "h.G/Say")).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    assert_eq!(c.format, "C#");
    assert!(c.definition.contains("string name = 3"), "{}", c.definition);
    assert!(c.definition.contains("repeated int64 ids = 7"));

    let kotlin = r#"
@Serializable
data class HelloRequest(
    @ProtoNumber(3) val name: String,
    @ProtoNumber(7) val ids: List<Long> = emptyList(),
)
"#;
    let c = convert(&with_method(kotlin, "h.G/Say")).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    assert_eq!(c.format, "Kotlin");
    assert!(
        c.definition.contains("repeated int64 ids = 7"),
        "{}",
        c.definition
    );
}

/// The same type, in each language: what comes out must be these fields.
fn assert_person(source: &str, language: &str) {
    let c = convert(&with_method(source, "people.v1.People/GetPerson")).unwrap();
    assert_eq!(c.format, language, "{}", c.definition);
    assert_eq!(c.certainty, Certainty::Guessed);
    let header = c.definition.lines().next().unwrap_or_default();
    assert!(header.contains(language), "{header}");
    assert!(c.definition.contains("guesses"), "{}", c.definition);
    let method = find_method(&c.pool, "people.v1.People/GetPerson").expect(&c.definition);
    assert_eq!(
        method.input().name(),
        "GetPersonRequest",
        "{}",
        c.definition
    );
    assert_eq!(
        method.output().name(),
        "GetPersonResponse",
        "{}",
        c.definition
    );
    let input = method.input();
    let field = |name: &str| {
        input
            .get_field_by_name(name)
            .unwrap_or_else(|| panic!("{name} in {}", c.definition))
    };
    // In the order written.
    assert_eq!(field("full_name").number(), 1, "{}", c.definition);
    assert!(
        c.definition.contains("string full_name = 1"),
        "{}",
        c.definition
    );
    assert!(field("tags").is_list(), "{}", c.definition);
    assert!(
        c.definition.contains("repeated string tags = 3"),
        "{}",
        c.definition
    );
    assert!(field("scores").is_map(), "{}", c.definition);
    assert!(matches!(field("address").kind(), Kind::Message(m) if m.name() == "Address"));
    assert!(matches!(field("status").kind(), Kind::Enum(e) if e.name() == "Status"));
    let address = c
        .pool
        .get_message_by_name("people.v1.Address")
        .expect(&c.definition);
    assert!(
        address.get_field_by_name("city").is_some(),
        "{}",
        c.definition
    );
    assert!(c.definition.contains("ACTIVE"), "{}", c.definition);
}

#[test]
fn typescript_types_are_read() {
    assert_person(
        r#"
export enum Status { ACTIVE, BLOCKED = 5 }
export interface Address { city: string; }
export interface GetPersonRequest {
  fullName: string;
  age?: number;
  tags: string[];
  scores: Record<string, number>;
  address: Address;
  status: Status;
}
export type GetPersonResponse = { ok: boolean };
"#,
        "TypeScript",
    );
}

#[test]
fn go_structs_are_read() {
    assert_person(
        r#"
package people

type Status int32

const (
	ACTIVE  Status = 0
	BLOCKED Status = 5
)

type Address struct {
	City string `json:"city"`
}

type GetPersonRequest struct {
	FullName string             `json:"fullName"`
	Age      *int32             `json:"age,omitempty"`
	Tags     []string           `json:"tags"`
	Scores   map[string]float64 `json:"scores"`
	Address  Address            `json:"address"`
	Status   Status             `json:"status"`
	hidden   string
	Skip     string             `json:"-"`
}

type GetPersonResponse struct {
	Ok bool `json:"ok"`
}
"#,
        "Go",
    );
}

#[test]
fn rust_structs_are_read() {
    assert_person(
        r#"
pub enum Status { Active, Blocked = 5 }
#[derive(Serialize)]
pub struct Address { pub city: String }
#[derive(Serialize)]
pub struct GetPersonRequest {
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub age: Option<i32>,
    pub tags: Vec<String>,
    pub scores: HashMap<String, f64>,
    pub address: Address,
    pub status: Status,
}
pub struct GetPersonResponse { pub ok: bool }
"#
        .replace("Active", "ACTIVE")
        .as_str(),
        "Rust",
    );
}

#[test]
fn csharp_classes_are_read() {
    assert_person(
        r#"
namespace People;
public enum Status { ACTIVE, BLOCKED = 5 }
public class Address { public string City { get; set; } }
public class GetPersonRequest {
    public string FullName { get; set; }
    public int? Age { get; set; }
    public List<string> Tags { get; set; } = new();
    public Dictionary<string, double> Scores { get; set; }
    public Address Address { get; set; }
    public Status Status { get; set; }
    public void Ignored() { }
}
public record GetPersonResponse(bool Ok);
"#,
        "C#",
    );
}

#[test]
fn java_classes_are_read() {
    assert_person(
        r#"
import java.util.*;
public enum Status { ACTIVE, BLOCKED }
public class Address { private String city; }
public class GetPersonRequest {
    private String fullName;
    private Integer age;
    private List<String> tags;
    private Map<String, Double> scores;
    private Address address;
    private Status status;
    public static final int LIMIT = 3;
    public String getFullName() { return fullName; }
}
public class GetPersonResponse { private boolean ok; }
"#,
        "Java",
    );
}

#[test]
fn kotlin_classes_are_read() {
    assert_person(
        r#"
enum class Status { ACTIVE, BLOCKED }
data class Address(val city: String)
data class GetPersonRequest(
    val fullName: String,
    val age: Int? = null,
    val tags: List<String> = emptyList(),
    val scores: Map<String, Double>,
    val address: Address,
    val status: Status,
)
data class GetPersonResponse(val ok: Boolean)
"#,
        "Kotlin",
    );
}

#[test]
fn python_classes_are_read() {
    assert_person(
        r#"
class Status(Enum):
    ACTIVE = 0
    BLOCKED = 5

@dataclass
class Address:
    city: str

@dataclass
class GetPersonRequest:
    full_name: str
    age: Optional[int] = None
    tags: List[str] = field(default_factory=list)
    scores: Dict[str, float] = field(default_factory=dict)
    address: Address = None
    status: Status = Status.ACTIVE

    def greet(self) -> str:
        return "hi"

class GetPersonResponse(BaseModel):
    ok: bool
"#,
        "Python",
    );
}

#[test]
fn swift_structs_are_read() {
    assert_person(
        r#"
enum Status: Int { case ACTIVE; case BLOCKED }
struct Address: Codable { let city: String }
struct GetPersonRequest: Codable {
    let fullName: String
    var age: Int?
    var tags: [String]
    var scores: [String: Double]
    let address: Address
    let status: Status
}
struct GetPersonResponse { let ok: Bool }
"#,
        "Swift",
    );
}

#[test]
fn cpp_structs_are_read() {
    assert_person(
        r#"
enum class Status { ACTIVE, BLOCKED };
struct Address { std::string city; };
struct GetPersonRequest {
    std::string full_name;
    std::optional<int32_t> age;
    std::vector<std::string> tags;
    std::map<std::string, double> scores;
    Address address;
    Status status;
};
struct GetPersonResponse { bool ok; };
"#,
        "C++",
    );
}

#[test]
fn dart_classes_are_read() {
    assert_person(
        r#"
enum Status { ACTIVE, BLOCKED }
class Address { final String city; }
class GetPersonRequest {
  final String fullName;
  final int? age;
  final List<String> tags;
  final Map<String, double> scores;
  final Address address;
  final Status status;
}
class GetPersonResponse { final bool ok; }
"#,
        "Dart",
    );
}

#[test]
fn inheritance_inline_objects_and_aliases_come_through() {
    let code = r#"
type Id = string;
type Mode = "fast" | "slow";
interface Base { id: Id; }
interface GetRequest extends Base {
  mode: Mode;
  nested: { a: string; b?: boolean };
  list: Array<{ c: string }>;
  labels: { [key: string]: string };
  when: Date;
  handler: (a: string) => void;
  extra: any;
  matrix: string[][];
}
"#;
    let c = convert(&with_method(code, "x.S/Get")).unwrap();
    let input = find_method(&c.pool, "x.S/Get")
        .expect(&c.definition)
        .input();
    let names: Vec<String> = input.fields().map(|f| f.name().to_owned()).collect();
    assert_eq!(
        names,
        [
            "id", "mode", "nested", "list", "labels", "when", "extra", "matrix"
        ],
        "{}",
        c.definition
    );
    assert!(c.definition.contains("string id = 1"), "{}", c.definition);
    assert!(c.definition.contains("string mode = 2"));
    assert!(input.get_field_by_name("labels").unwrap().is_map());
    assert!(c.definition.contains("google.protobuf.Timestamp when"));
    assert!(
        c.definition
            .contains("import \"google/protobuf/timestamp.proto\"")
    );
    assert!(c.definition.contains("google.protobuf.Value extra"));
    assert!(
        c.notes.iter().any(|n| n.contains("nests lists")),
        "{:?}",
        c.notes
    );
}

#[test]
fn a_type_the_source_does_not_describe_becomes_an_empty_message() {
    let code = "interface GetRequest { other: Elsewhere; }";
    let c = convert(&with_method(code, "x.S/Get")).unwrap();
    assert!(
        c.notes.iter().any(|n| n.contains("Elsewhere")),
        "{:?}",
        c.notes
    );
    assert!(c.pool.get_message_by_name("x.Elsewhere").is_some());
}

#[test]
fn a_json_example_becomes_a_message_in_key_order() {
    let source = r#"{
      "zeta": "a",
      "firstName": "Ada",
      "userID": 7,
      "big": 5000000000,
      "ratio": 0.5,
      "active": true,
      "nothing": null,
      "tags": ["a", "b"],
      "empty": [],
      "address": {"city": "Ankara", "zip": null},
      "orders": [{"id": 1}, {"id": 2, "note": "x"}],
      "byId": {"1f-2a": {"n": 1}, "9c-0b": {"n": 2}},
      "grid": [[1], [2]],
      "weird-key": "v",
      "at": "2026-11-01T10:00:00Z"
    }"#;
    let c = convert(&with_method(source, "shop.v1.Orders/Create")).unwrap();
    assert_eq!(c.certainty, Certainty::Guessed);
    assert_eq!(c.format, "a JSON example");
    let method = find_method(&c.pool, "shop.v1.Orders/Create").expect(&c.definition);
    let input = method.input();
    assert_eq!(input.name(), "CreateRequest");
    assert_eq!(method.output().name(), "CreateResponse");
    assert_eq!(method.output().fields().len(), 0);
    let line = |text: &str| assert!(c.definition.contains(text), "{text} in {}", c.definition);
    line("string zeta = 1");
    line("string first_name = 2");
    line("int32 user_id = 3 [json_name = \"userID\"]");
    line("int64 big = 4");
    line("double ratio = 5");
    line("bool active = 6");
    line("optional string nothing = 7");
    line("repeated string tags = 8");
    line("repeated string empty = 9");
    line("Address address = 10");
    line("repeated Order orders = 11");
    line("map<string, ByI");
    line("repeated GridList grid");
    line("string weird_key");
    let order = c.pool.get_message_by_name("shop.v1.Order").unwrap();
    assert!(order.get_field_by_name("note").is_some());
    assert!(
        c.notes.iter().any(|n| n.contains("looks like a time")),
        "{:?}",
        c.notes
    );
    assert!(
        c.notes.iter().any(|n| n.contains("nothing")),
        "{:?}",
        c.notes
    );

    // The same JSON now fits the definition, under its own key names.
    let mut request = apix_core::Request::new(apix_core::Method::Post, "grpc://x:1");
    request.body = apix_core::Body::Json(
        r#"{"firstName": "Ada", "userID": 7, "weird-key": "v", "orders": [{"id": 1}]}"#.into(),
    );
    apix_grpc::messages(&request, &method).expect("the example fits");
}

#[test]
fn a_json_answer_and_lists_make_streams() {
    let c = convert(&ConvertInput {
        source: r#"[{"n": 1}, {"n": 2}]"#,
        response: Some(r#"[{"sum": 3}]"#),
        method: Some("calc.Calc/Sum"),
    })
    .unwrap();
    let method = find_method(&c.pool, "calc.Calc/Sum").expect(&c.definition);
    assert!(method.is_client_streaming() && method.is_server_streaming());
    assert!(method.output().get_field_by_name("sum").is_some());
}

#[test]
fn variables_in_a_json_example_are_no_trouble() {
    let c = convert(&with_method(
        r#"{"id": {{id}}, "name": "{{name}}"}"#,
        "a.B/C",
    ))
    .unwrap();
    assert!(c.definition.contains("int32 id = 1"), "{}", c.definition);
    assert!(c.definition.contains("string name = 2"));
}

#[test]
fn json_without_a_method_still_converts_and_says_what_is_missing() {
    let c = convert(&input(r#"{"a": 1}"#)).unwrap();
    assert!(methods(&c.pool).is_empty());
    assert!(
        c.notes.iter().any(|n| n.contains("what to call")),
        "{:?}",
        c.notes
    );
}

#[test]
fn what_cannot_be_read_says_so() {
    let error = convert(&input("just some words")).unwrap_err().to_string();
    assert!(error.contains("no types could be read"), "{error}");
    let error = convert(&input("{ not json")).unwrap_err().to_string();
    assert!(error.contains("not valid JSON"), "{error}");
    assert!(convert(&input("   ")).is_err());
}

#[test]
fn method_paths_in_generated_code_make_the_calls() {
    let code = r#"
type HelloRequest struct { Name string }
type HelloReply struct { Greeting string }
err := c.cc.Invoke(ctx, "/hello.Greeter/SayHello", in, out, opts...)
"#;
    let c = convert(&input(code)).unwrap();
    let method = find_method(&c.pool, "hello.Greeter/SayHello").expect(&c.definition);
    assert_eq!(method.input().name(), "HelloRequest");
    assert_eq!(method.output().name(), "HelloReply");
}

#[test]
fn comparing_tells_numbers_types_and_missing_fields_apart() {
    let mine = compile_definition(
        r#"syntax = "proto3"; package p;
        service S { rpc M(Guess) returns (Out); rpc Gone(Guess) returns (Out); }
        message Guess { string name = 1; int32 age = 2; string gone = 3; }
        message Out {}"#,
        &[],
    )
    .unwrap();
    let server = compile_definition(
        r#"syntax = "proto3"; package p;
        service S { rpc M(Real) returns (stream Reply); }
        message Real { string name = 4; double age = 2; string extra = 3; }
        message Reply { string text = 1; }"#,
        &[],
    )
    .unwrap();
    let differences = compare(&mine, &server).join("\n");
    assert!(differences.contains("no method p.S/Gone"), "{differences}");
    assert!(differences.contains("streaming differs"), "{differences}");
    assert!(
        differences.contains("name: number 1 here, 4 on the server"),
        "{differences}"
    );
    assert!(
        differences.contains("age: int32 here, double on the server"),
        "{differences}"
    );
    assert!(
        differences.contains("gone: the server has no such field (its number 3 is `extra`)"),
        "{differences}"
    );
    assert!(
        differences.contains("the server also has `extra` = 3"),
        "{differences}"
    );
    // Nothing was known about Out, so there is nothing to tell about it.
    assert!(!differences.contains("text"), "{differences}");
    assert!(compare(&mine, &mine).is_empty());
}

fn generated(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/generated")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// What real generators wrote for `orders.proto` must come back as it: every
/// call and field, with the original's numbers and types.
#[test]
fn real_generated_code_gives_back_the_original_service() {
    let original =
        compile_definition(&String::from_utf8(generated("orders.proto")).unwrap(), &[]).unwrap();
    for name in [
        "orders.pb",
        "orders_pb2.py",
        "OrdersOuterClass.java",
        "Orders.cs",
        "orders.pb.cc",
        "Orders.php",
        "orders_pb.ts",
    ] {
        let c =
            convert_bytes(&generated(name), None, None).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(c.certainty, Certainty::Exact, "{name}");
        assert_eq!(compare(&c.pool, &original), Vec::<String>::new(), "{name}");
        assert_eq!(compare(&original, &c.pool), Vec::<String>::new(), "{name}");
        let back = compile_definition(&c.definition, &[]).unwrap();
        assert_eq!(
            compare(&back, &original),
            Vec::<String>::new(),
            "{name}: the printed text"
        );
        let create = find_method(&back, "shop.v1.Orders/Chat").unwrap();
        assert!(
            create.is_client_streaming() && create.is_server_streaming(),
            "{name}"
        );
    }
}

#[test]
fn real_ts_proto_output_gives_numbers_types_and_calls() {
    let text = String::from_utf8(generated("orders_ts_proto.ts")).unwrap();
    let c = convert(&input(&text)).unwrap();
    assert_eq!(
        c.certainty,
        Certainty::NumbersFromSource,
        "{}",
        c.definition
    );
    let original =
        compile_definition(&String::from_utf8(generated("orders.proto")).unwrap(), &[]).unwrap();
    // A oneof reads as plain fields here, which go the same on the wire.
    assert_eq!(
        compare(&c.pool, &original),
        Vec::<String>::new(),
        "{}",
        c.definition
    );
    assert_eq!(methods(&c.pool).len(), 4);
    assert!(c.notes.is_empty(), "{:?}", c.notes);
}

#[test]
fn generated_code_without_a_description_points_at_the_one_with_it() {
    let text = String::from_utf8(generated("orders_head.pb.h")).unwrap();
    let error = convert(&input(&text)).unwrap_err().to_string();
    assert!(error.contains("_pb2.py"), "{error}");
    let calls_only = "class OrdersStub(object):\n    def __init__(self, channel):\n        self.Create = channel.unary_unary(\n            '/shop.v1.Orders/Create')\n";
    let error = convert(&input(calls_only)).unwrap_err().to_string();
    assert!(error.contains("names calls"), "{error}");
}

#[test]
fn a_definition_round_trips_through_a_request_file() {
    let text = "name = \"x\"\nurl = \"grpc://x:1\"\n\n[grpc]\nmethod = \"a.B/C\"\ndefinition = '''\nsyntax = \"proto3\";\n'''\nguessed = true\n";
    let request: apix_core::Request = toml::from_str(text).unwrap();
    let call = request.grpc.clone().unwrap();
    assert!(call.guessed);
    assert!(call.definition.contains("proto3"));
    let back: apix_core::Request =
        toml::from_str(&toml::to_string_pretty(&request).unwrap()).unwrap();
    assert_eq!(request, back);
    // Left out when there is nothing to say.
    let plain = apix_core::GrpcCall {
        method: "a.B/C".into(),
        ..Default::default()
    };
    let written = toml::to_string_pretty(&plain).unwrap();
    assert!(
        !written.contains("definition") && !written.contains("guessed"),
        "{written}"
    );
}
