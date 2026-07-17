use std::time::Duration;

pub struct Config<Id> {
	// the nodes identity - who am I?
	pub me: Id,
	
	//how many peers a message is forwarded to in a given round (derived from the published works on HyPerView)
	pub fanout: usize,

	//how long a node waits between gossip rounds
	pub round_interval: Duration, 
	
	// How many rounds a payload keeps being reshared before it is finished distributing 
	pub max_rounds: u32,
	pub passive_capacity: usize,
	//Active Random Walk Length - max hops a ForwardJoin travels (HyParView used 6)
	pub active_walk_length: u32,
	//Passive Random Walk Length - hop at which a joining node enters the passive view (HyParView used 3)
	pub passive_walk_length: u32,
}

impl<Id> Config<Id> { 
	//config gossip defaults from HyPerView Article Specs
	pub fn new(me: Id) -> Self {
		Config {
			me, 
			fanout: 3,
			round_interval: Duration::from_secs(1),
			max_rounds: 5,
			passive_capacity: 30,
			active_walk_length: 6,
			passive_walk_length: 3,
		}
	}
}

